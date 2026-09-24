//! Environment detection, `init` and `doctor` (spec §63-§70).
//!
//! The CLI only parses arguments; all decision-making lives here.

pub mod agents;
pub mod agents_md;
pub mod project;
pub mod skills;
pub mod tools;

use std::path::Path;

use serde::Serialize;

use crate::config::{load_effective, ConfigPaths};
use crate::core::project::find_project_root;
use agents::AgentDetection;
use tools::ToolDetection;

/// Snapshot of the host environment used by `init` and `doctor`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnvironmentReport {
    pub os: String,
    pub arch: String,
    pub git: bool,
    pub agents: Vec<AgentDetection>,
    pub tools: Vec<ToolDetection>,
}

/// Probe OS, architecture, git, agents and tools.
pub fn environment_report(root: Option<&Path>) -> EnvironmentReport {
    let tools = tools::detect_tools(root);
    let git = tools::find_tool(&tools, "Git").map(|t| t.found).unwrap_or(false);
    EnvironmentReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        git,
        agents: agents::detect_agents(),
        tools,
    }
}

/// Doctor check status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Optional,
    Missing,
}

impl CheckStatus {
    /// Text marker used by the human-readable doctor output (spec §70).
    pub fn marker(self) -> &'static str {
        match self {
            CheckStatus::Ok => "[OK]",
            CheckStatus::Optional => "[--]",
            CheckStatus::Missing => "[X]",
        }
    }
}

/// A single doctor row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

/// Full doctor report. `exit_code` is 0 unless a required dependency is
/// missing or the configuration is irreparably corrupt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub exit_code: i32,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    /// Render the `[OK]/[--]/[X] component` text format.
    pub fn render_text(&self) -> String {
        let mut out = String::from("NodKray Doctor\n\n");
        for check in &self.checks {
            out.push_str(check.status.marker());
            out.push(' ');
            out.push_str(&check.name);
            if !check.detail.is_empty() {
                out.push_str(" — ");
                out.push_str(&check.detail);
            }
            out.push('\n');
        }
        out
    }
}

/// Run diagnostics. Never fails for optional integrations (spec §70).
pub fn doctor(paths: &ConfigPaths, cwd: &Path) -> DoctorReport {
    let root = find_project_root(cwd);
    let env = environment_report(Some(&root.root));

    let mut checks: Vec<DoctorCheck> = Vec::new();
    let mut ok = true;
    let mut exit_code = 0;

    checks.push(DoctorCheck {
        name: "NodKray".to_string(),
        status: CheckStatus::Ok,
        detail: env!("CARGO_PKG_VERSION").to_string(),
    });

    // Required: Git.
    let git_found = env
        .tools
        .iter()
        .find(|t| t.name == "Git")
        .map(|t| t.found)
        .unwrap_or(false);
    if git_found {
        checks.push(DoctorCheck {
            name: "Git".to_string(),
            status: CheckStatus::Ok,
            detail: String::new(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "Git".to_string(),
            status: CheckStatus::Missing,
            detail: "required; install git".to_string(),
        });
        ok = false;
        exit_code = 4;
    }

    // SQLite capability (bundled, therefore always present).
    checks.push(DoctorCheck {
        name: "SQLite".to_string(),
        status: CheckStatus::Ok,
        detail: format!("bundled {}", rusqlite::version()),
    });

    // Configuration.
    let global_exists = paths.global_config_file().is_file();
    let project_exists = paths.project_config_file(&root.root).is_file();
    match load_effective(paths, Some(&root.root)) {
        Ok(_) => checks.push(DoctorCheck {
            name: "Configuration".to_string(),
            status: CheckStatus::Ok,
            detail: format!(
                "global: {}, project: {}",
                yes_no(global_exists),
                yes_no(project_exists)
            ),
        }),
        Err(err) => {
            checks.push(DoctorCheck {
                name: "Configuration".to_string(),
                status: CheckStatus::Missing,
                detail: format!("{} [{}]", err.message(), err.category().as_str()),
            });
            ok = false;
            if exit_code == 0 {
                exit_code = err.exit_code();
            }
        }
    }

    // Agents: detected = OK, missing = optional.
    for agent in &env.agents {
        let (status, detail) = if agent.found {
            (
                CheckStatus::Ok,
                agent
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            )
        } else {
            (
                CheckStatus::Optional,
                format!("{} not found; optional", agent.executable),
            )
        };
        checks.push(DoctorCheck {
            name: format!("agent:{}", agent.name),
            status,
            detail,
        });
    }

    // Optional integrations (Git already covered above).
    for tool in &env.tools {
        if tool.name == "Git" {
            continue;
        }
        let (status, detail) = if tool.found {
            (
                CheckStatus::Ok,
                tool.detail
                    .clone()
                    .or_else(|| tool.path.as_ref().map(|p| p.display().to_string()))
                    .unwrap_or_default(),
            )
        } else {
            (CheckStatus::Optional, "not found; optional".to_string())
        };
        checks.push(DoctorCheck {
            name: tool.name.clone(),
            status,
            detail,
        });
    }

    // Project rules.
    let rules = project::detect_rules(&root.root);
    if rules.is_empty() {
        checks.push(DoctorCheck {
            name: "Project rules".to_string(),
            status: CheckStatus::Optional,
            detail: "none detected".to_string(),
        });
    } else {
        let names: Vec<String> = rules.iter().map(|r| r.name.clone()).collect();
        checks.push(DoctorCheck {
            name: "Project rules".to_string(),
            status: CheckStatus::Ok,
            detail: names.join(", "),
        });
    }

    DoctorReport {
        ok,
        exit_code,
        checks,
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_does_not_fail_when_optional_tools_are_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");

        let report = doctor(&paths, &repo);

        let config_ok = report
            .checks
            .iter()
            .any(|c| c.name == "Configuration" && c.status == CheckStatus::Ok);
        assert!(config_ok, "config must be usable with defaults");

        // Optional integrations never fail the process: a missing tool must be
        // reported as Optional, not Missing.
        assert!(report
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Missing)
            .all(|c| c.name == "Git"));

        // Exit code mirrors required-dependency availability only.
        let git_present = tools::find_in_path("git").is_some();
        assert_eq!(report.exit_code, if git_present { 0 } else { 4 });
    }

    #[test]
    fn doctor_reports_configuration_failure_on_corrupt_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        std::fs::create_dir_all(&paths.config_home).expect("config home");
        std::fs::write(paths.global_config_file(), "{ not yaml: [").expect("write");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");

        let report = doctor(&paths, &repo);
        assert!(!report.ok);
        assert_eq!(report.exit_code, 3);
    }
}
