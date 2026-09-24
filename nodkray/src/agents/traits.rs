//! Agent adapter interface, Agent Contract (§76) and Output Contract (§77).
//!
//! Phase 2 implements a real Claude adapter and a configurable Generic adapter
//! (§129). The remaining providers (cursor/opencode/codex/pi) are registry
//! stubs: they only answer capability/detection queries, and their headless
//! command reports "not implemented" so the workflow can fall back to generic.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::tools::find_in_path;

/// Capability matrix of an adapter (spec §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    pub interactive: bool,
    pub headless: bool,
    pub json_output: bool,
    pub mcp: bool,
    pub skills: bool,
    pub worktree: bool,
    pub system_prompt: bool,
    pub stdin: bool,
    pub session_resume: bool,
}

/// Result of probing for an adapter executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DetectionResult {
    pub found: bool,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
}

/// Everything NodKray tells a worker (spec §76, §77).
#[derive(Debug, Clone, Default)]
pub struct AgentRequest {
    pub task_id: String,
    pub project_root: PathBuf,
    pub workflow: String,
    pub role: String,
    pub description: String,
    pub constraints: Vec<String>,
    /// Previewed memory context, never full entries (spec §133).
    pub memory_context: Vec<String>,
    /// YOLO only relaxes worker permissions, never the workflow (spec §71).
    pub yolo: bool,
}

/// A process to execute for a worker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub env: Vec<(String, String)>,
}

/// Raw process output.
#[derive(Debug, Clone, Default)]
pub struct AgentOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Parsed worker result, matching the Output Contract (spec §77).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResult {
    pub status: String,
    pub summary: String,
    pub changed_files: Vec<String>,
    pub tests_run: Vec<String>,
    pub notes: Vec<String>,
    pub blocking_issues: Vec<String>,
}

/// Whether the result came from a valid Output Contract JSON document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedAgentResult {
    #[serde(flatten)]
    pub result: AgentResult,
    pub parsed_json: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

/// An external coding agent driver (spec §7.1).
pub trait AgentAdapter {
    /// Stable provider id (`cursor`, `opencode`, `claude`, `codex`, `pi`, `generic`).
    fn id(&self) -> &'static str;
    /// Executable (or command head) looked up on `PATH`.
    fn executable(&self) -> &str;
    fn detect(&self) -> DetectionResult;
    fn capabilities(&self) -> AgentCapabilities;
    fn version(&self) -> NodkrayResult<String>;
    /// Command used for headless execution (spec §76).
    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec>;
    /// Parse worker output into the Output Contract (spec §77).
    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult>;
}

/// Maximum length of a memory-context preview line.
pub const CONTEXT_PREVIEW_LEN: usize = 200;

fn preview(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    trimmed.chars().take(max).collect::<String>() + "…"
}

/// Build the structured Agent Contract prompt (spec §76).
pub fn build_prompt(request: &AgentRequest) -> String {
    let memory: Vec<String> = request
        .memory_context
        .iter()
        .map(|entry| preview(entry, CONTEXT_PREVIEW_LEN))
        .collect();

    let contract = serde_json::json!({
        "task_id": request.task_id,
        "project": { "root": request.project_root.display().to_string() },
        "workflow": { "type": request.workflow },
        "role": { "name": request.role },
        "constraints": request.constraints,
        "memory_context": memory,
        "instructions": "Complete the task described below by editing files in the current working directory. Run the project's tests when possible.",
        "validation": { "required": true }
    });

    format!(
        "You are a NodKray worker agent.\n\n\
AGENT CONTRACT:\n{contract}\n\n\
TASK:\n{description}\n\n\
Return ONLY a JSON object in this exact Output Contract shape:\n\
{{\"status\":\"completed\",\"summary\":\"...\",\"changed_files\":[],\"tests_run\":[],\"notes\":[],\"blocking_issues\":[]}}\n\
Do not wrap it in prose or code fences.\n",
        contract = serde_json::to_string_pretty(&contract).unwrap_or_else(|_| "{}".to_string()),
        description = request.description,
    )
}

/// Extract and parse an Output Contract JSON object from free text.
pub fn parse_output_contract(text: &str) -> Option<AgentResult> {
    for candidate in json_candidates(text) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&candidate) {
            if let Some(result) = agent_result_from_value(&value) {
                return Some(result);
            }
        }
    }
    None
}

fn json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        candidates.push(trimmed.to_string());
    }
    // ```json ... ``` fenced block.
    for fence in ["```json", "```"] {
        if let Some(start) = trimmed.find(fence) {
            let rest = &trimmed[start + fence.len()..];
            if let Some(end) = rest.find("```") {
                candidates.push(rest[..end].trim().to_string());
            }
        }
    }
    // Outermost braces.
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            candidates.push(trimmed[start..=end].to_string());
        }
    }
    candidates
}

fn agent_result_from_value(value: &serde_json::Value) -> Option<AgentResult> {
    let status = value.get("status")?.as_str()?.to_string();
    let strings = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(|entry| entry.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(AgentResult {
        status,
        summary: value
            .get("summary")
            .and_then(|entry| entry.as_str())
            .unwrap_or_default()
            .to_string(),
        changed_files: strings("changed_files"),
        tests_run: strings("tests_run"),
        notes: strings("notes"),
        blocking_issues: strings("blocking_issues"),
    })
}

/// Default parser: valid Output Contract JSON is used as-is; otherwise the raw
/// stdout becomes the summary and the status is `completed` with a warning
/// (spec: never crash on non-JSON output).
pub fn default_parse_result(output: &AgentOutput) -> ParsedAgentResult {
    if let Some(result) = parse_output_contract(&output.stdout) {
        return ParsedAgentResult {
            result,
            parsed_json: true,
            raw: None,
        };
    }

    let raw = preview(&output.stdout, 4000);
    let mut notes = vec!["worker did not emit a valid Output Contract JSON document".to_string()];
    if let Some(code) = output.exit_code {
        notes.push(format!("worker exit code: {code}"));
    }
    if !output.stderr.trim().is_empty() {
        notes.push(format!("stderr: {}", preview(&output.stderr, 500)));
    }
    ParsedAgentResult {
        result: AgentResult {
            status: "completed".to_string(),
            summary: raw.clone(),
            changed_files: Vec::new(),
            tests_run: Vec::new(),
            notes,
            blocking_issues: Vec::new(),
        },
        parsed_json: false,
        raw: Some(raw),
    }
}

/// Registry of available adapters.
pub struct AgentRegistry {
    adapters: Vec<Box<dyn AgentAdapter>>,
}

impl AgentRegistry {
    pub fn new(adapters: Vec<Box<dyn AgentAdapter>>) -> Self {
        Self { adapters }
    }

    /// Registry with all built-in adapters; `generic_command` configures §129.
    pub fn with_defaults(generic_command: Option<String>) -> Self {
        Self::new(vec![
            Box::new(crate::agents::cursor::adapter()),
            Box::new(crate::agents::opencode::adapter()),
            Box::new(crate::agents::codex::adapter()),
            Box::new(crate::agents::pi::adapter()),
            Box::new(crate::agents::claude::adapter()),
            Box::new(crate::agents::generic::adapter(generic_command)),
        ])
    }

    /// Look up an adapter by id.
    pub fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.id() == id)
            .map(|adapter| adapter.as_ref())
    }

    /// All adapter ids, in registration order.
    pub fn ids(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|adapter| adapter.id()).collect()
    }

    /// Pick a runnable adapter: the requested provider when it can run headless,
    /// otherwise the Generic adapter (phase 2 fallback, §129).
    pub fn select(&self, provider: &str) -> NodkrayResult<&dyn AgentAdapter> {
        if let Some(adapter) = self.get(provider) {
            if adapter.capabilities().headless && adapter.detect().found {
                return Ok(adapter);
            }
            tracing::warn!(
                provider,
                found = adapter.detect().found,
                "provider cannot run headless here; falling back to generic"
            );
        }
        self.get("generic").ok_or_else(|| {
            NodkrayError::agent(
                "AGENT_NOT_FOUND",
                format!("no adapter available for provider `{provider}`"),
            )
        })
    }
}

/// A detected-but-not-yet-implemented provider (cursor/opencode/codex/pi).
pub struct StubAdapter {
    id: &'static str,
    executable: &'static str,
    capabilities: AgentCapabilities,
}

impl StubAdapter {
    pub fn new(id: &'static str, executable: &'static str) -> Self {
        Self {
            id,
            executable,
            capabilities: AgentCapabilities {
                interactive: true,
                headless: false,
                worktree: true,
                ..AgentCapabilities::default()
            },
        }
    }
}

impl AgentAdapter for StubAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn executable(&self) -> &str {
        self.executable
    }

    fn detect(&self) -> DetectionResult {
        match find_in_path(self.executable) {
            Some(path) => DetectionResult {
                found: true,
                path: Some(path),
                version: None,
            },
            None => DetectionResult::default(),
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        self.capabilities.clone()
    }

    fn version(&self) -> NodkrayResult<String> {
        Err(NodkrayError::agent(
            "AGENT_VERSION_UNSUPPORTED",
            format!("{} version reporting is not implemented", self.id),
        ))
    }

    fn non_interactive_command(&self, _request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        Err(NodkrayError::agent(
            "AGENT_HEADLESS_UNSUPPORTED",
            format!(
                "{} headless execution is not implemented in phase 2",
                self.id
            ),
        ))
    }

    fn parse_result(&self, output: &AgentOutput) -> NodkrayResult<ParsedAgentResult> {
        Ok(default_parse_result(output))
    }
}

/// Look up a program path for an adapter executable, supporting absolute paths.
pub fn resolve_executable(program: &str) -> Option<PathBuf> {
    let as_path = Path::new(program);
    if as_path.is_absolute() || program.contains('/') {
        return as_path.is_file().then(|| as_path.to_path_buf());
    }
    find_in_path(program)
}

pub(crate) fn env_for_request(request: &AgentRequest) -> Vec<(String, String)> {
    let mut env = Vec::new();
    env.push(("NODKRAY_TASK_ID".to_string(), request.task_id.clone()));
    env.push((
        "NODKRAY_PROJECT_ROOT".to_string(),
        request.project_root.display().to_string(),
    ));
    env.push(("NODKRAY_WORKFLOW".to_string(), request.workflow.clone()));
    env.push(("NODKRAY_ROLE".to_string(), request.role.clone()));
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AgentRequest {
        AgentRequest {
            task_id: "task_1".to_string(),
            project_root: PathBuf::from("/repo"),
            workflow: "ST".to_string(),
            role: "default".to_string(),
            description: "do the thing".to_string(),
            constraints: vec!["no new deps".to_string()],
            memory_context: vec!["a very long memory".to_string()],
            yolo: false,
        }
    }

    #[test]
    fn prompt_embeds_contract_and_output_contract_hint() {
        let prompt = build_prompt(&request());
        assert!(prompt.contains("task_1"));
        assert!(prompt.contains("\"type\": \"ST\""));
        assert!(prompt.contains("Output Contract"));
        assert!(prompt.contains("do the thing"));
    }

    #[test]
    fn parses_plain_and_fenced_output_contracts() {
        let plain = r#"{"status":"completed","summary":"ok","changed_files":["a"],"tests_run":[],"notes":[],"blocking_issues":[]}"#;
        let parsed = parse_output_contract(plain).expect("plain");
        assert_eq!(parsed.status, "completed");
        assert_eq!(parsed.changed_files, vec!["a".to_string()]);

        let fenced = format!("here you go:\n```json\n{plain}\n```\n");
        assert!(parse_output_contract(&fenced).is_some());
    }

    #[test]
    fn non_json_output_becomes_completed_with_warning() {
        let output = AgentOutput {
            stdout: "I did some work but forgot the JSON".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        };
        let parsed = default_parse_result(&output);
        assert_eq!(parsed.result.status, "completed");
        assert!(!parsed.parsed_json);
        assert!(parsed.result.notes.iter().any(|note| note.contains("Output Contract")));
        assert!(parsed.raw.is_some());
    }

    #[test]
    fn registry_falls_back_to_generic_when_provider_missing() {
        let registry = AgentRegistry::with_defaults(Some("echo".to_string()));
        let selected = registry.select("codex").expect("select");
        if crate::installer::tools::find_in_path("codex").is_some() {
            assert_eq!(selected.id(), "codex");
        } else {
            assert_eq!(selected.id(), "generic");
        }
    }

    #[test]
    fn claude_is_headless() {
        let registry = AgentRegistry::with_defaults(None);
        let claude = registry.get("claude").expect("claude");
        assert!(claude.capabilities().headless);
        assert!(claude.capabilities().json_output);
    }
}
