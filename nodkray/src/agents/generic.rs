//! Generic adapter for unsupported agents (spec §129).

use std::path::PathBuf;

use super::traits::{AgentAdapter, AgentCapabilities, AgentRequest, ProcessSpec};
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::tools::find_in_path;

#[derive(Debug, Clone)]
pub struct GenericAdapter {
    pub command: String,
    pub args: Vec<String>,
}

impl Default for GenericAdapter {
    fn default() -> Self {
        Self {
            command: "my-agent".to_string(),
            args: vec!["--headless".to_string()],
        }
    }
}

impl AgentAdapter for GenericAdapter {
    fn name(&self) -> &'static str {
        "generic"
    }

    fn executable(&self) -> &'static str {
        "my-agent"
    }

    fn detect(&self) -> Option<PathBuf> {
        find_in_path(&self.command)
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            interactive: true,
            headless: true,
            json_output: false,
            mcp: false,
            skills: false,
            worktree: true,
            system_prompt: false,
            stdin: true,
            session_resume: false,
        }
    }

    fn interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        self.command_with(request, false)
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        self.command_with(request, true)
    }
}

impl GenericAdapter {
    fn command_with(&self, request: &AgentRequest, headless: bool) -> NodkrayResult<ProcessSpec> {
        if self.command.trim().is_empty() {
            return Err(NodkrayError::configuration(
                "GENERIC_AGENT_UNCONFIGURED",
                "generic agent requires a command",
            ));
        }
        let mut args = self.args.clone();
        args.extend(request.extra_args.iter().cloned());
        if headless {
            args.push(request.prompt.clone());
        }
        Ok(ProcessSpec {
            program: self.command.clone(),
            args,
            cwd: request.working_directory.clone(),
            stdin: if request.prompt.is_empty() {
                None
            } else if headless {
                None
            } else {
                Some(request.prompt.clone())
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_headless_command_from_config() {
        let adapter = GenericAdapter {
            command: "my-agent".into(),
            args: vec!["--headless".into()],
        };
        let spec = adapter
            .non_interactive_command(&AgentRequest {
                prompt: "ship it".into(),
                working_directory: PathBuf::from("/repo"),
                yolo: false,
                extra_args: Vec::new(),
            })
            .expect("cmd");
        assert_eq!(spec.program, "my-agent");
        assert_eq!(spec.args, ["--headless", "ship it"]);
    }
}
