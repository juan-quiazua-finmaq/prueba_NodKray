//! Cursor adapter (spec §9.1).

use super::traits::{
    build_prompt, default_parse_result, env_for_request, resolve_executable, AgentAdapter,
    AgentCapabilities, AgentOutput, AgentRequest, DetectionResult, ParsedAgentResult, ProcessSpec,
};
use crate::error::NodkrayResult;

pub struct CursorAdapter;

pub fn adapter() -> CursorAdapter {
    CursorAdapter
}

impl AgentAdapter for CursorAdapter {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn executable(&self) -> &str {
        "cursor-agent"
    }

    fn detect(&self) -> DetectionResult {
        let path = resolve_executable("cursor-agent").or_else(|| resolve_executable("agent"));
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
            json_output: false,
            mcp: true,
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
        let mut args = vec!["-p".to_string(), build_prompt(request)];
        if request.yolo {
            args.push("--force".to_string());
        }
        Ok(ProcessSpec {
            program: self.executable().to_string(),
            args,
            stdin: None,
            env: env_for_request(request),
        })
    }

    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult> {
        Ok(default_parse_result(output))
    }
}
