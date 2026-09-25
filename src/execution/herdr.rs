//! Herdr execution backend (spec §14). Optional; workflows never call Herdr directly.

use std::path::Path;

use super::console::ConsoleBackend;
use super::traits::{
    ExecutionBackend, WorkerEvent, WorkerOutcome, WorkerSpec, Workspace, WorkspaceRequest,
};
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::tools::find_in_path;

#[derive(Debug, Default, Clone)]
pub struct HerdrBackend;

impl HerdrBackend {
    pub fn detect() -> Option<std::path::PathBuf> {
        find_in_path("herdr")
    }

    /// Binary exists **and** implements the `herdr run` process-wrapper contract.
    pub fn runner_usable() -> bool {
        Self::detect().is_some() && crate::installer::probes::herdr_runner_usable()
    }
}

impl ExecutionBackend for HerdrBackend {
    fn name(&self) -> &'static str {
        "herdr"
    }

    fn available(&self) -> bool {
        Self::runner_usable()
    }

    fn create_workspace(&self, request: &WorkspaceRequest) -> NodkrayResult<Workspace> {
        ConsoleBackend.create_workspace(request)
    }

    fn spawn_worker(
        &self,
        spec: &WorkerSpec,
        cwd: &Path,
        on_event: &mut dyn FnMut(WorkerEvent),
    ) -> NodkrayResult<WorkerOutcome> {
        let herdr = Self::detect().filter(|_| crate::installer::probes::herdr_runner_usable()).ok_or_else(|| {
            NodkrayError::dependency(
                "HERDR_UNAVAILABLE",
                "Herdr is not a usable process backend (`herdr run` missing); use console",
            )
        })?;
        let mut wrapped = spec.clone();
        let mut args = vec![
            "run".to_string(),
            "--agent".to_string(),
            spec.role.clone(),
            "--workdir".to_string(),
            cwd.display().to_string(),
            "--".to_string(),
            spec.process.program.clone(),
        ];
        args.extend(spec.process.args.iter().cloned());
        wrapped.process.program = herdr.display().to_string();
        wrapped.process.args = args;
        ConsoleBackend.spawn_worker(&wrapped, cwd, on_event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_stable() {
        assert_eq!(HerdrBackend.name(), "herdr");
    }
}
