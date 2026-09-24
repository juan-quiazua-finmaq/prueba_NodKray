//! Agent registry (spec §150). Adding an adapter does not touch DecisionEngine.

use serde::Serialize;

use super::claude::ClaudeAdapter;
use super::codex::CodexAdapter;
use super::cursor::CursorAdapter;
use super::generic::GenericAdapter;
use super::opencode::OpenCodeAdapter;
use super::pi::PiAdapter;
use super::traits::{AgentAdapter, AgentCapabilities, DetectionResult};
use crate::error::{NodkrayError, NodkrayResult};

/// Snapshot used by `nodkray agent list|inspect`.
#[derive(Debug, Clone, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub executable: String,
    pub detection: DetectionResult,
    pub capabilities: AgentCapabilities,
}

pub fn known_ids() -> &'static [&'static str] {
    &["cursor", "opencode", "claude", "codex", "pi", "generic"]
}

pub fn adapter_by_id(id: &str) -> NodkrayResult<Box<dyn AgentAdapter>> {
    match id.trim().to_ascii_lowercase().as_str() {
        "cursor" => Ok(Box::new(CursorAdapter)),
        "opencode" => Ok(Box::new(OpenCodeAdapter)),
        "claude" => Ok(Box::new(ClaudeAdapter)),
        "codex" => Ok(Box::new(CodexAdapter)),
        "pi" => Ok(Box::new(PiAdapter)),
        "generic" => Ok(Box::new(GenericAdapter::default())),
        other => Err(NodkrayError::user_input(
            "UNKNOWN_AGENT",
            format!("unknown agent '{other}'"),
        )),
    }
}

pub fn inspect(id: &str) -> NodkrayResult<AgentInfo> {
    let adapter = adapter_by_id(id)?;
    Ok(info_from(&*adapter))
}

pub fn list() -> Vec<AgentInfo> {
    known_ids()
        .iter()
        .filter_map(|id| inspect(id).ok())
        .collect()
}

fn info_from(adapter: &dyn AgentAdapter) -> AgentInfo {
    AgentInfo {
        id: adapter.name().to_string(),
        executable: adapter.executable().to_string(),
        detection: adapter.detection(),
        capabilities: adapter.capabilities(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lists_every_built_in_adapter() {
        let ids: Vec<_> = list().into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["cursor", "opencode", "claude", "codex", "pi", "generic"]);
    }

    #[test]
    fn unknown_agent_is_user_input() {
        let err = inspect("gemini").expect_err("unknown");
        assert_eq!(err.code(), "UNKNOWN_AGENT");
    }
}
