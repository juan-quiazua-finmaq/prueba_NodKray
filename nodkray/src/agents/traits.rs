//! Common agent adapter interface (spec §7.1, §8, §76-§77, §102).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::NodkrayResult;
use crate::installer::tools::find_in_path;

/// Result of probing whether an agent is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectionResult {
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Capability flags advertised by an adapter (spec §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl AgentCapabilities {
    /// Back-compat alias used by the fase-2 scaffold (`structured_output`).
    pub fn structured_output(&self) -> bool {
        self.json_output
    }
}

/// Prompt and workspace handed to an adapter.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub prompt: String,
    pub working_directory: PathBuf,
    pub yolo: bool,
    pub extra_args: Vec<String>,
}

/// Spawn specification produced by an adapter. Execution backends run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
}

/// Worker output contract (spec §77).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentResult {
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub tests_run: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub blocking_issues: Vec<String>,
}

/// An external coding agent that NodKray can drive.
pub trait AgentAdapter: Send + Sync {
    /// Stable provider name (`cursor`, `opencode`, `claude`, `codex`, `pi`).
    fn name(&self) -> &'static str;
    /// Executable looked up on `PATH`.
    fn executable(&self) -> &'static str;
    /// Return the resolved executable path when available.
    fn detect(&self) -> Option<PathBuf> {
        find_in_path(self.executable())
    }
    /// Advertised capabilities.
    fn capabilities(&self) -> AgentCapabilities;

    fn detection(&self) -> DetectionResult {
        let path = self.detect();
        let version = path.as_ref().and_then(|p| probe_version(p));
        DetectionResult {
            installed: path.is_some(),
            path,
            version,
        }
    }

    fn version(&self) -> NodkrayResult<String> {
        match self.detection().version {
            Some(version) => Ok(version),
            None => Err(crate::error::NodkrayError::agent(
                "AGENT_VERSION_UNAVAILABLE",
                format!("{} version could not be determined", self.name()),
            )),
        }
    }

    fn interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        Ok(self.base_spec(request, Vec::new()))
    }

    fn non_interactive_command(&self, request: &AgentRequest) -> NodkrayResult<ProcessSpec> {
        Ok(self.base_spec(request, vec![request.prompt.clone()]))
    }

    fn base_spec(&self, request: &AgentRequest, extra: Vec<String>) -> ProcessSpec {
        let program = self
            .detect()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| self.executable().to_string());
        let mut args = extra;
        args.extend(request.extra_args.iter().cloned());
        ProcessSpec {
            program,
            args,
            cwd: request.working_directory.clone(),
            stdin: None,
        }
    }
}

pub fn probe_version(executable: &Path) -> Option<String> {
    let output = std::process::Command::new(executable)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

pub fn require_detected(adapter: &dyn AgentAdapter) -> NodkrayResult<PathBuf> {
    adapter.detect().ok_or_else(|| {
        crate::error::NodkrayError::agent(
            "AGENT_NOT_FOUND",
            format!("{} executable was not found", adapter.name()),
        )
    })
}
