//! SDD workflow coordination (spec §23, §26-§27). Spec-Kit owns the stages.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};
use crate::spec::constitution::{self, ConstitutionReport, ConstitutionStatus};
use crate::spec::speckit::{SpecKitAdapter, SpecKitDetection, SpecKitStage};
use crate::agents::traits::ProcessSpec;

/// Prepared SDD run. Execution of each `ProcessSpec` is the execution backend's job.
#[derive(Debug, Clone, Serialize)]
pub struct SddPlan {
    pub constitution: ConstitutionReport,
    pub speckit: SpecKitDetection,
    pub stages: Vec<SddStagePlan>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SddStagePlan {
    pub stage: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// Prepare an SDD run: reuse constitution, require Spec-Kit, do not fall back to ST.
pub fn prepare(
    project_root: &Path,
    create_constitution: bool,
    extra_stages: &[SpecKitStage],
) -> NodkrayResult<SddPlan> {
    let detection = SpecKitAdapter::detect(Some(project_root));
    if !detection.installed {
        return Err(NodkrayError::dependency(
            "SPECKIT_NOT_FOUND",
            "SDD requires Spec-Kit; refusing to downgrade to ST",
        ));
    }
    let constitution = constitution::ensure(project_root, create_constitution)?;
    if constitution.status == ConstitutionStatus::Absent {
        return Err(NodkrayError::configuration(
            "CONSTITUTION_MISSING",
            format!(
                "SDD needs {}; create it once with an explicit init, do not regenerate per task",
                constitution.path
            ),
        ));
    }

    let adapter = SpecKitAdapter::locate(Some(project_root))?;
    let mut stages: Vec<SpecKitStage> = SpecKitStage::core_cycle().to_vec();
    for extra in extra_stages {
        if !stages.iter().any(|s| s == extra) {
            stages.push(*extra);
        }
    }

    let planned = stages
        .into_iter()
        .map(|stage| {
            let spec: ProcessSpec = adapter.command(stage, project_root, &[]);
            SddStagePlan {
                stage: stage.as_str().to_string(),
                program: spec.program,
                args: spec.args,
                cwd: spec.cwd,
            }
        })
        .collect();

    Ok(SddPlan {
        constitution,
        speckit: detection,
        stages: planned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_sdd_without_speckit() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = prepare(tmp.path(), false, &[]).expect_err("sdd blocked");
        if crate::installer::tools::find_in_path("specify").is_some() {
            assert_eq!(err.code(), "CONSTITUTION_MISSING");
        } else {
            assert_eq!(err.code(), "SPECKIT_NOT_FOUND");
        }
    }

    #[test]
    fn reuses_existing_constitution_when_specify_is_on_path() {
        if crate::installer::tools::find_in_path("specify").is_none() {
            return;
        }
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".specify/memory")).expect("mkdir");
        std::fs::write(
            tmp.path().join(".specify/memory/constitution.md"),
            "keep\n",
        )
        .expect("write");
        let plan = prepare(tmp.path(), true, &[]).expect("plan");
        assert_eq!(plan.constitution.status, ConstitutionStatus::Reused);
        assert_eq!(std::fs::read_to_string(tmp.path().join(".specify/memory/constitution.md")).unwrap(), "keep\n");
        assert_eq!(plan.stages.len(), 5);
        assert_eq!(plan.stages[0].stage, "specify");
        assert_eq!(plan.stages[4].stage, "converge");
    }
}
