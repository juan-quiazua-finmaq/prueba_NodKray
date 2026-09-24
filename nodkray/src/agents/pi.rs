//! Pi coding-agent adapter (spec §9.5).

use super::traits::{
    require_detected, AgentAdapter, AgentCapabilities, AgentRequest, ProcessSpec,
};
use crate::error::NodkrayResult;

#[derive(Debug, Default, Clone)]
pub struct PiAdapter;

impl AgentAdapter for PiAdapter {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn executable(&self) -> &'static str {
        "pi"
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

    fn interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        let program = require_detected(self)?;
        Ok(ProcessSpec {
            program: program.display().to_string(),
            args: request.extra_args.clone(),
            cwd: request.working_directory.clone(),
            stdin: None,
        })
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        let program = require_detected(self)?;
        let mut args = vec!["--print".to_string(), request.prompt.clone()];
        args.extend(request.extra_args.iter().cloned());
        Ok(ProcessSpec {
            program: program.display().to_string(),
            args,
            cwd: request.working_directory.clone(),
            stdin: None,
        })
    }
}
