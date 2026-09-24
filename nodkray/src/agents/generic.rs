//! Generic adapter (spec §129): runs an arbitrary, user-configured command.
//!
//! The command comes from `agents.generic.command` (or `NODKRAY_GENERIC_COMMAND`)
//! and is split on whitespace. The Agent Contract prompt is fed on stdin, and
//! stdout is parsed with the shared Output Contract parser.

use super::traits::{
    build_prompt, default_parse_result, env_for_request, resolve_executable, AgentAdapter,
    AgentCapabilities, AgentOutput, AgentRequest, DetectionResult, ParsedAgentResult, ProcessSpec,
};
use crate::error::{NodkrayError, NodkrayResult};

/// Generic, command-driven worker adapter.
pub struct GenericAdapter {
    command: Option<Vec<String>>,
}

/// Registry entry for the generic provider.
pub fn adapter(command: Option<String>) -> GenericAdapter {
    GenericAdapter {
        command: command.map(|raw| {
            raw.split_whitespace()
                .map(str::to_string)
                .collect::<Vec<String>>()
        }),
    }
}

impl GenericAdapter {
    pub fn from_command(command: Option<String>) -> Self {
        adapter(command)
    }
}

impl AgentAdapter for GenericAdapter {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn executable(&self) -> &str {
        self.command
            .as_ref()
            .and_then(|parts| parts.first())
            .map(String::as_str)
            .unwrap_or("")
    }

    fn detect(&self) -> DetectionResult {
        match self.command.as_ref().and_then(|parts| parts.first()) {
            Some(program) => match resolve_executable(program) {
                Some(path) => DetectionResult {
                    found: true,
                    path: Some(path),
                    version: None,
                },
                None => DetectionResult::default(),
            },
            None => DetectionResult::default(),
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            interactive: false,
            headless: self.command.is_some(),
            json_output: false,
            worktree: true,
            stdin: true,
            ..AgentCapabilities::default()
        }
    }

    fn version(&self) -> NodkrayResult<String> {
        Err(NodkrayError::agent(
            "AGENT_VERSION_UNSUPPORTED",
            "generic adapter has no version command",
        ))
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        let parts = self.command.as_ref().ok_or_else(|| {
            NodkrayError::agent(
                "AGENT_NOT_CONFIGURED",
                "generic adapter is not configured (set agents.generic.command or NODKRAY_GENERIC_COMMAND)",
            )
        })?;
        let (program, args) = parts.split_first().ok_or_else(|| {
            NodkrayError::agent("AGENT_NOT_CONFIGURED", "generic adapter command is empty")
        })?;
        Ok(ProcessSpec {
            program: program.clone(),
            args: args.to_vec(),
            stdin: Some(build_prompt(request)),
            env: env_for_request(request),
        })
    }

    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult> {
        Ok(default_parse_result(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_generic_is_not_headless() {
        let adapter = adapter(None);
        assert!(!adapter.capabilities().headless);
        assert!(adapter.non_interactive_command(&AgentRequest::default()).is_err());
    }

    #[test]
    fn configured_generic_splits_command_and_feeds_prompt_on_stdin() {
        let adapter = adapter(Some("/bin/echo --flag".to_string()));
        assert!(adapter.capabilities().headless);
        let spec = adapter
            .non_interactive_command(&AgentRequest {
                description: "hello".to_string(),
                ..AgentRequest::default()
            })
            .expect("spec");
        assert_eq!(spec.program, "/bin/echo");
        assert_eq!(spec.args, vec!["--flag".to_string()]);
        assert!(spec.stdin.expect("stdin").contains("hello"));
    }
}
