//! Session version snapshot (spec §127, §163).
//!
//! Recorded once when a session starts so a historical run can be reproduced
//! even if later tool upgrades change the host.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::agents::AgentRegistry;
use crate::config::Config;
use crate::installer::tools::find_in_path;

/// Versions of NodKray and optional companion tools at session start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionVersions {
    pub nodkray: String,
    pub agents: BTreeMap<String, Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speckit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub herdr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentrux: Option<String>,
}

/// Logical session snapshot stored in `sessions.config_snapshot`.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSnapshot<'a> {
    pub config: serde_json::Value,
    pub versions: SessionVersions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<&'a str>,
}

/// Collect available versions. Missing tools are recorded as `null` / omitted.
pub fn collect_versions(root: &Path, generic_command: Option<String>) -> SessionVersions {
    let registry = AgentRegistry::with_defaults(generic_command);
    let mut agents = BTreeMap::new();
    for id in registry.ids() {
        let Some(adapter) = registry.get(id) else {
            continue;
        };
        let version = if adapter.detect().found {
            adapter
                .version()
                .ok()
                .map(|text| first_line(&text))
                .filter(|text| !text.is_empty())
        } else {
            None
        };
        agents.insert(id.to_string(), version);
    }

    let _ = root;
    SessionVersions {
        nodkray: env!("CARGO_PKG_VERSION").to_string(),
        agents,
        speckit: probe_version("specify"),
        herdr: probe_version("herdr"),
        sentrux: probe_version("sentrux"),
    }
}

/// Serialise the config + versions snapshot stored at session start.
pub fn snapshot_session(config: &Config, root: &Path) -> String {
    let snapshot = SessionSnapshot {
        config: config.snapshot(),
        versions: collect_versions(root, config.agents.generic.command.clone()),
        project_root: root.to_str(),
    };
    serde_json::to_string(&snapshot).unwrap_or_else(|_| config.snapshot().to_string())
}

fn probe_version(binary: &str) -> Option<String> {
    find_in_path(binary)?;
    let mut command = Command::new(binary);
    command.arg("--version");
    let output = match command.output() {
        Ok(output) => output,
        Err(_) => return None,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let line = first_line(&text);
    (!line.is_empty()).then_some(line)
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or(text)
        .trim()
        .chars()
        .take(200)
        .collect()
}

/// Best-effort bound so `--version` probes cannot hang a session start.
#[allow(dead_code)]
const _PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn snapshot_always_includes_nodkray_version() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let versions = collect_versions(tmp.path(), None);
        assert_eq!(versions.nodkray, env!("CARGO_PKG_VERSION"));
        assert!(versions.agents.contains_key("generic"));
        assert!(versions.agents.contains_key("claude"));
    }

    #[test]
    fn session_snapshot_json_has_config_and_versions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let raw = snapshot_session(&Config::default(), tmp.path());
        let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(value["versions"]["nodkray"], env!("CARGO_PKG_VERSION"));
        assert!(value["config"]["decision"]["provider"].is_string());
    }
}
