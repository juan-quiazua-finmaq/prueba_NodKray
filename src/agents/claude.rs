//! Claude adapter (spec §9.3). Real headless execution in phase 2:
//!
//! ```text
//! claude -p <prompt> --output-format json [--dangerously-skip-permissions]
//! ```
//!
//! Claude wraps its answer in a JSON envelope with a `result` field, so the
//! adapter unwraps that first and then applies the shared Output Contract parser.

use super::traits::{
    build_prompt, default_parse_result, env_for_request, parse_output_contract, resolve_executable,
    AgentAdapter, AgentCapabilities, AgentOutput, AgentRequest, DetectionResult, ParsedAgentResult,
    ProcessSpec,
};
use crate::error::NodkrayResult;

/// Claude Code adapter.
pub struct ClaudeAdapter;

/// Registry entry for the Claude provider.
pub fn adapter() -> ClaudeAdapter {
    ClaudeAdapter
}

impl AgentAdapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn executable(&self) -> &str {
        "claude"
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
            mcp: true,
            skills: true,
            worktree: true,
            system_prompt: true,
            stdin: false,
            session_resume: true,
        }
    }

    fn version(&self) -> NodkrayResult<String> {
        let output = std::process::Command::new(self.executable())
            .arg("--version")
            .output()
            .map_err(|err| {
                crate::error::NodkrayError::agent("AGENT_VERSION_FAILED", err.to_string())
            })?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        let mut args = vec![
            "-p".to_string(),
            build_prompt(request),
            "--output-format".to_string(),
            "json".to_string(),
        ];
        // YOLO only relaxes worker permissions; it never changes the workflow (§71).
        if request.yolo {
            args.push("--dangerously-skip-permissions".to_string());
        }
        Ok(ProcessSpec {
            program: self.executable().to_string(),
            args,
            stdin: None,
            env: env_for_request(request),
        })
    }

    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult> {
        // Claude emits an envelope: { ..., "result": "<agent text>" }.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(output.stdout.trim()) {
            if let Some(text) = value.get("result").and_then(|result| result.as_str()) {
                if let Some(result) = parse_output_contract(text) {
                    return Ok(ParsedAgentResult {
                        result,
                        parsed_json: true,
                        raw: None,
                    });
                }
            }
            // Envelope JSON but no inner Output Contract: treat the envelope as raw.
        }
        Ok(default_parse_result(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_includes_yolo_flag_only_when_requested() {
        let adapter = ClaudeAdapter;
        let mut request = AgentRequest {
            task_id: "task_1".to_string(),
            workflow: "ST".to_string(),
            role: "default".to_string(),
            description: "x".to_string(),
            ..AgentRequest::default()
        };
        let spec = adapter.non_interactive_command(&request).expect("spec");
        assert_eq!(spec.program, "claude");
        assert!(spec.args.contains(&"--output-format".to_string()));
        assert!(!spec.args.contains(&"--dangerously-skip-permissions".to_string()));

        request.yolo = true;
        let spec = adapter.non_interactive_command(&request).expect("spec");
        assert!(spec.args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn unwraps_claude_result_envelope() {
        let envelope = serde_json::json!({
            "type": "result",
            "result": "{\"status\":\"completed\",\"summary\":\"done\",\"changed_files\":[],\"tests_run\":[],\"notes\":[],\"blocking_issues\":[]}"
        })
        .to_string();
        let output = AgentOutput {
            stdout: envelope,
            stderr: String::new(),
            exit_code: Some(0),
        };
        let parsed = ClaudeAdapter.parse_result(&output).expect("parse");
        assert!(parsed.parsed_json);
        assert_eq!(parsed.result.summary, "done");
    }
}
