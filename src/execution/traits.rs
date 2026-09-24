//! Execution backend interface (spec §13-§15, §102).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::agents::ProcessSpec;
use crate::error::NodkrayResult;

/// Request to isolate a worker in a workspace (git worktree).
#[derive(Debug, Clone)]
pub struct WorkspaceRequest {
    pub root: PathBuf,
    pub task_id: String,
    pub token: String,
    pub base_branch: Option<String>,
}

/// A prepared workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub path: PathBuf,
    pub branch: String,
    pub created: bool,
}

/// Everything needed to spawn one worker process.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    pub task_id: String,
    pub role: String,
    pub agent: String,
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    pub process: ProcessSpec,
}

/// A worker lifecycle event persisted to `task_events` (spec §78).
#[derive(Debug, Clone, Serialize)]
pub struct WorkerEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

impl WorkerEvent {
    pub fn new(event: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            event: event.into(),
            payload,
        }
    }
}

/// Terminal status of a worker process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Completed,
    Failed,
}

impl WorkerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkerStatus::Completed => "completed",
            WorkerStatus::Failed => "failed",
        }
    }
}

/// Outcome of a worker process.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerOutcome {
    pub status: WorkerStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Where and how workers are launched (spec §13).
pub trait ExecutionBackend {
    /// Stable backend name (`console`, `herdr`).
    fn name(&self) -> &'static str;
    /// Whether this backend can currently be used on this host.
    fn available(&self) -> bool;
    /// Create (or reuse) an isolated workspace for a worker (spec §16).
    fn create_workspace(&self, request: &WorkspaceRequest) -> NodkrayResult<Workspace>;
    /// Spawn a worker and wait for it, emitting lifecycle events (spec §78).
    fn spawn_worker(
        &self,
        spec: &WorkerSpec,
        cwd: &Path,
        on_event: &mut dyn FnMut(WorkerEvent),
    ) -> NodkrayResult<WorkerOutcome>;
}
