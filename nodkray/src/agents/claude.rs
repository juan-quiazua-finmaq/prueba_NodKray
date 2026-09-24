//! Claude Code adapter (spec §9.3). Workers use `-p/--print`.

use super::traits::{
    require_detected, AgentAdapter, AgentCapabilities, AgentRequest, ProcessSpec,
};
use crate::error::NodkrayResult;

#[derive(Debug, Default, Clone)]
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn executable(&self) -> &'static str {
        "claude"
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
        let mut args = vec!["-p".to_string(), request.prompt.clone()];
        if request.yolo {
            args.push("--dangerously-skip-permissions".to_string());
        }
        args.extend(request.extra_args.iter().cloned());
        Ok(ProcessSpec {
            program: program.display().to_string(),
            args,
            cwd: request.working_directory.clone(),
            stdin: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn headless_uses_print_flag() {
        let request = AgentRequest {
            prompt: "do the thing".into(),
            working_directory: PathBuf::from("/repo"),
            yolo: true,
            extra_args: Vec::new(),
        };
        if ClaudeAdapter.detect().is_none() {
            return;
        }
        let spec = ClaudeAdapter.non_interactive_command(&request).expect("cmd");
        assert_eq!(spec.args[0], "-p");
        assert!(spec.args.contains(&"--dangerously-skip-permissions".to_string()));
    }
}
