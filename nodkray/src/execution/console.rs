//! Console execution backend (spec §15). Fase 2 owns process spawn; this
//! implementation only satisfies the shared trait so Herdr can fall back.

use super::traits::{
    ExecutionBackend, WorkerHandle, WorkerRequest, WorkerStatus, Workspace, WorkspaceRequest,
};
use super::worktree;
use crate::error::NodkrayResult;

#[derive(Debug, Default, Clone)]
pub struct ConsoleBackend;

impl ExecutionBackend for ConsoleBackend {
    fn name(&self) -> &'static str {
        "console"
    }

    fn available(&self) -> bool {
        true
    }

    fn create_workspace(&self, request: &WorkspaceRequest) -> NodkrayResult<Workspace> {
        let path = worktree::create_worktree(
            &request.project_root,
            &request.worker_id,
            request.branch.as_deref(),
        )?;
        Ok(Workspace {
            worker_id: request.worker_id.clone(),
            path,
        })
    }

    fn spawn_worker(&self, request: &WorkerRequest) -> NodkrayResult<WorkerHandle> {
        // Fase 2 owns the real process lifecycle. This backend records the handle
        // so DAG/worktree orchestration can be tested without spawning agents.
        Ok(WorkerHandle {
            id: request.worker_id.clone(),
            role: request.role.clone(),
            workspace: request.workspace.clone(),
        })
    }

    fn status(&self, _worker: &WorkerHandle) -> NodkrayResult<WorkerStatus> {
        Ok(WorkerStatus::Created)
    }

    fn stop(&self, _worker: &WorkerHandle) -> NodkrayResult<()> {
        Ok(())
    }

    fn destroy(&self, _worker: &WorkerHandle) -> NodkrayResult<()> {
        Ok(())
    }
}
