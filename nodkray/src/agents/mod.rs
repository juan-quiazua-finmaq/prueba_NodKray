//! Agent adapters (spec §7-§12, §150).

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod generic;
pub mod opencode;
pub mod pi;
pub mod registry;
pub mod traits;

pub use registry::{adapter_by_id, inspect, list, AgentInfo};
pub use traits::{AgentAdapter, AgentCapabilities, AgentRequest, AgentResult, ProcessSpec};
