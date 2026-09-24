//! Cursor agent adapter (spec §9.1).

use std::path::PathBuf;

use super::traits::{
    require_detected, AgentAdapter, AgentCapabilities, AgentRequest, ProcessSpec,
};
use crate::error::NodkrayResult;
use crate::installer::tools::find_in_path;

#[derive(Debug, Default, Clone)]
pub struct CursorAdapter;

impl AgentAdapter for CursorAdapter {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn executable(&self) -> &'static str {
        "cursor-agent"
    }

    fn detect(&self) -> Option<PathBuf> {
        find_in_path("cursor-agent").or_else(|| find_in_path("agent"))
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
            args.push("--force".to_string());
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
    fn capabilities_include_mcp_and_headless() {
        let caps = CursorAdapter.capabilities();
        assert!(caps.headless);
        assert!(caps.mcp);
        assert!(caps.worktree);
    }

    #[test]
    fn missing_binary_is_agent_error() {
        let request = AgentRequest {
            prompt: "hi".into(),
            working_directory: PathBuf::from("/tmp"),
            yolo: false,
            extra_args: Vec::new(),
        };
        if CursorAdapter.detect().is_some() {
            let spec = CursorAdapter.non_interactive_command(&request).expect("cmd");
            assert_eq!(spec.args[0], "-p");
            assert_eq!(spec.args[1], "hi");
        } else {
            let err = CursorAdapter
                .non_interactive_command(&request)
                .expect_err("missing");
            assert_eq!(err.code(), "AGENT_NOT_FOUND");
        }
    }
}
