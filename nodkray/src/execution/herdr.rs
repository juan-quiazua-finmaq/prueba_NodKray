//! Herdr execution backend (spec §14). Optional; workflows never call Herdr directly.

use super::traits::{
    ExecutionBackend, WorkerHandle, WorkerRequest, WorkerStatus, Workspace, WorkspaceRequest,
};
use super::worktree;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::tools::find_in_path;

#[derive(Debug, Default, Clone)]
pub struct HerdrBackend;

impl HerdrBackend {
    pub fn detect() -> Option<std::path::PathBuf> {
        find_in_path("herdr")
    }
}

impl ExecutionBackend for HerdrBackend {
    fn name(&self) -> &'static str {
        "herdr"
    }

    fn available(&self) -> bool {
        Self::detect().is_some()
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
        let herdr = Self::detect().ok_or_else(|| {
            NodkrayError::dependency(
                "HERDR_UNAVAILABLE",
                "Herdr is not installed; use the console backend or install herdr",
            )
        })?;
        let mut command = std::process::Command::new(herdr);
        command
            .arg("run")
            .arg("--agent")
            .arg(&request.role)
            .arg("--workdir")
            .arg(&request.workspace)
            .arg("--")
            .arg(&request.program)
            .args(&request.args);
        let status = command.status().map_err(|err| {
            NodkrayError::execution("HERDR_SPAWN_FAILED", err.to_string())
        })?;
        if !status.success() {
            return Err(NodkrayError::execution(
                "HERDR_WORKER_FAILED",
                format!("herdr exited with {status}"),
            ));
        }
        Ok(WorkerHandle {
            id: request.worker_id.clone(),
            role: request.role.clone(),
            workspace: request.workspace.clone(),
        })
    }

    fn status(&self, _worker: &WorkerHandle) -> NodkrayResult<WorkerStatus> {
        Ok(WorkerStatus::Completed)
    }

    fn stop(&self, _worker: &WorkerHandle) -> NodkrayResult<()> {
        Ok(())
    }

    fn destroy(&self, _worker: &WorkerHandle) -> NodkrayResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_when_herdr_missing() {
        if HerdrBackend::detect().is_none() {
            assert!(!HerdrBackend.available());
        }
    }

    #[test]
    fn name_is_stable() {
        assert_eq!(HerdrBackend.name(), "herdr");
    }
}
