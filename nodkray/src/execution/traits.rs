//! Common execution backend interface (spec §13, §102).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NodkrayError, NodkrayResult};

/// Request to isolate a worker workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceRequest {
    pub project_root: PathBuf,
    pub worker_id: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub worker_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorkerRequest {
    pub worker_id: String,
    pub role: String,
    pub workspace: PathBuf,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHandle {
    pub id: String,
    pub role: String,
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatus {
    Created,
    Started,
    Working,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

/// Where and how a worker process is launched.
pub trait ExecutionBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn available(&self) -> bool;

    fn create_workspace(&self, request: &WorkspaceRequest) -> NodkrayResult<Workspace> {
        let _ = request;
        Err(not_implemented(self.name(), "create_workspace"))
    }

    fn spawn_worker(&self, request: &WorkerRequest) -> NodkrayResult<WorkerHandle> {
        let _ = request;
        Err(not_implemented(self.name(), "spawn_worker"))
    }

    fn send(&self, worker: &WorkerHandle, input: &str) -> NodkrayResult<()> {
        let _ = (worker, input);
        Err(not_implemented(self.name(), "send"))
    }

    fn status(&self, worker: &WorkerHandle) -> NodkrayResult<WorkerStatus> {
        let _ = worker;
        Err(not_implemented(self.name(), "status"))
    }

    fn stop(&self, worker: &WorkerHandle) -> NodkrayResult<()> {
        let _ = worker;
        Err(not_implemented(self.name(), "stop"))
    }

    fn destroy(&self, worker: &WorkerHandle) -> NodkrayResult<()> {
        let _ = worker;
        Err(not_implemented(self.name(), "destroy"))
    }
}

fn not_implemented(backend: &str, operation: &str) -> NodkrayError {
    NodkrayError::execution(
        "BACKEND_OPERATION_UNSUPPORTED",
        format!("{backend} does not implement {operation} in this build"),
    )
}
