//! Pi adapter (spec §9.5).

use super::traits::{
    build_prompt, default_parse_result, env_for_request, resolve_executable, AgentAdapter,
    AgentCapabilities, AgentOutput, AgentRequest, DetectionResult, ParsedAgentResult, ProcessSpec,
};
use crate::error::NodkrayResult;

pub struct PiAdapter;

pub fn adapter() -> PiAdapter {
    PiAdapter
}

impl AgentAdapter for PiAdapter {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn executable(&self) -> &str {
        "pi"
    }

    fn detect(&self) -> DetectionResult {
        let path = resolve_executable(self.executable());
        DetectionResult {
            found: path.is_some(),
            path,
            version: None,
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            interactive: true,
            headless: true,
            json_output: true,
            mcp: false,
            skills: true,
            worktree: true,
            system_prompt: true,
            stdin: true,
            session_resume: true,
        }
    }

    fn version(&self) -> NodkrayResult<String> {
        Ok(self.detect().version.unwrap_or_else(|| "unknown".into()))
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        Ok(ProcessSpec {
            program: self.executable().to_string(),
            args: vec!["--print".to_string(), build_prompt(request)],
            stdin: None,
            env: env_for_request(request),
        })
    }

    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult> {
        Ok(default_parse_result(output))
    }
}
