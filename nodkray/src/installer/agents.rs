//! Agent detection on `PATH` (spec §9, §63, FR-002).

use std::path::PathBuf;

use serde::Serialize;

use super::tools::find_in_path;

/// Known providers and their executable names.
pub const KNOWN_AGENTS: &[(&str, &str)] = &[
    ("cursor", "cursor-agent"),
    ("opencode", "opencode"),
    ("claude", "claude"),
    ("codex", "codex"),
    ("pi", "pi"),
];

/// Result of probing a single agent provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentDetection {
    pub name: String,
    pub executable: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// Probe every known agent; ordering is stable.
pub fn detect_agents() -> Vec<AgentDetection> {
    KNOWN_AGENTS
        .iter()
        .map(|(name, executable)| {
            let path = find_in_path(executable);
            AgentDetection {
                name: (*name).to_string(),
                executable: (*executable).to_string(),
                found: path.is_some(),
                path,
            }
        })
        .collect()
}

/// Executable name for a provider, if known.
pub fn agent_executable(name: &str) -> Option<&'static str> {
    KNOWN_AGENTS
        .iter()
        .find(|(provider, _)| *provider == name)
        .map(|(_, executable)| *executable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_all_known_agents_without_panicking() {
        let detected = detect_agents();
        assert_eq!(detected.len(), KNOWN_AGENTS.len());
        assert!(detected.iter().any(|d| d.name == "codex"));
    }

    #[test]
    fn cursor_uses_cursor_agent_executable() {
        assert_eq!(agent_executable("cursor"), Some("cursor-agent"));
        assert_eq!(agent_executable("nope"), None);
    }
}
