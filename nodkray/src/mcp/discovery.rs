//! MCP discovery (spec §40-§43, §94-§96). Detect only; do not wrap tools.

use std::path::Path;

use serde::Serialize;

use crate::config::Config;
use crate::core::roles;
use crate::installer::tools::find_in_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpServer {
    pub name: String,
    pub installed: bool,
    pub configured: bool,
    pub available: bool,
}

const KNOWN: &[(&str, &str, &str)] = &[
    ("serena", "serena", ".serena"),
    ("codegraph", "codegraph", ".codegraph"),
    ("sentrux", "sentrux", ".sentrux"),
];

pub fn discover(root: Option<&Path>, config: &Config) -> Vec<McpServer> {
    KNOWN
        .iter()
        .map(|(name, binary, marker)| {
            let installed = find_in_path(binary).is_some()
                || root.map(|r| r.join(marker).exists()).unwrap_or(false);
            let configured = match *name {
                "serena" => config.integrations.serena.enabled,
                "codegraph" => config.integrations.codegraph.enabled,
                "sentrux" => config.integrations.sentrux.enabled,
                _ => false,
            };
            McpServer {
                name: (*name).to_string(),
                installed,
                configured,
                available: installed && configured,
            }
        })
        .collect()
}

/// MCP names selected for a worker role (`all`, `none`, or explicit).
pub fn select_for_role(config: &Config, role: &str, discovered: &[McpServer]) -> Vec<String> {
    let requested = roles::mcp_for_role(config, role);
    if requested.is_empty() || requested.iter().any(|v| v == "all") {
        return discovered
            .iter()
            .filter(|s| s.available)
            .map(|s| s.name.clone())
            .collect();
    }
    if requested.iter().any(|v| v == "none") {
        return Vec::new();
    }
    requested
        .into_iter()
        .filter(|name| discovered.iter().any(|s| s.name == *name && s.available))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn reports_known_servers() {
        let config = Config::default();
        let servers = discover(None, &config);
        let names: Vec<_> = servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["serena", "codegraph", "sentrux"]);
    }

    #[test]
    fn role_none_selects_nothing() {
        let mut config = Config::default();
        config.agents.workers.backend.mcp = vec!["none".into()];
        let discovered = vec![McpServer {
            name: "serena".into(),
            installed: true,
            configured: true,
            available: true,
        }];
        assert!(select_for_role(&config, "backend", &discovered).is_empty());
    }
}
