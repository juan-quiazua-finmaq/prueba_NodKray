//! Convenience snapshots over [`AgentRegistry`] for `nodkray agent`.

use serde::Serialize;

use super::traits::{AgentCapabilities, AgentRegistry, DetectionResult};
use crate::error::{NodkrayError, NodkrayResult};

#[derive(Debug, Clone, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub executable: String,
    pub detection: DetectionResult,
    pub capabilities: AgentCapabilities,
}

pub fn list() -> Vec<AgentInfo> {
    let registry = AgentRegistry::with_defaults(None);
    registry
        .ids()
        .into_iter()
        .filter_map(|id| inspect(id).ok())
        .collect()
}

pub fn inspect(id: &str) -> NodkrayResult<AgentInfo> {
    let registry = AgentRegistry::with_defaults(None);
    let adapter = registry.get(id).ok_or_else(|| {
        NodkrayError::user_input("UNKNOWN_AGENT", format!("unknown agent '{id}'"))
    })?;
    Ok(AgentInfo {
        id: adapter.id().to_string(),
        executable: adapter.executable().to_string(),
        detection: adapter.detect(),
        capabilities: adapter.capabilities(),
    })
}
