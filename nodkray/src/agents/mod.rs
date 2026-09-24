//! Agent adapters (spec §7-§12). Phase 2 implements Claude + Generic and
//! registers cursor/opencode/codex/pi as detection stubs.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod generic;
pub mod opencode;
pub mod pi;
pub mod traits;

pub use traits::{
    build_prompt, parse_output_contract, AgentAdapter, AgentCapabilities, AgentOutput,
    AgentRegistry, AgentRequest, AgentResult, DetectionResult, ParsedAgentResult, ProcessSpec,
};
