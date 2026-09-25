//! SDD workflow coordination (spec §23, §26-§27). Spec-Kit owns the stages.

use std::path::Path;

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};
use crate::spec::constitution::{self, ConstitutionReport, ConstitutionStatus};
use crate::spec::speckit::{SpecKitAdapter, SpecKitDetection, SpecKitStage};

/// Prepared SDD run. Each stage is executed by the configured worker.
#[derive(Debug, Clone, Serialize)]
pub struct SddPlan {
    pub constitution: ConstitutionReport,
    pub speckit: SpecKitDetection,
    pub stages: Vec<SddStagePlan>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SddStagePlan {
    pub stage: String,
    pub slash: String,
    pub skill: String,
    pub prompt: String,
}

/// Prepare an SDD run: reuse constitution, require Spec-Kit CLI, do not fall back to ST.
pub fn prepare(
    project_root: &Path,
    create_constitution: bool,
    extra_stages: &[SpecKitStage],
    description: &str,
) -> NodkrayResult<SddPlan> {
    let detection = SpecKitAdapter::detect(Some(project_root));
    if detection.executable.is_none() {
        return Err(NodkrayError::dependency(
            "SPECKIT_NOT_FOUND",
            "SDD requires Spec-Kit (`specify` on PATH); refusing to downgrade to ST",
        ));
    }
    let constitution = constitution::ensure(project_root, create_constitution)?;
    if constitution.status == ConstitutionStatus::Absent {
        return Err(NodkrayError::configuration(
            "CONSTITUTION_MISSING",
            format!(
                "SDD needs {}; create it once with `nodkray init`, do not regenerate per task",
                constitution.path
            ),
        ));
    }

    let _adapter = SpecKitAdapter::locate(Some(project_root))?;
    let mut stages: Vec<SpecKitStage> = SpecKitStage::core_cycle().to_vec();
    for extra in extra_stages {
        if !stages.iter().any(|s| s == extra) {
            stages.push(*extra);
        }
    }

    let planned = stages
        .into_iter()
        .map(|stage| SddStagePlan {
            stage: stage.as_str().to_string(),
            slash: stage.slash_command().to_string(),
            skill: stage.skill_name().to_string(),
            prompt: SpecKitAdapter::stage_prompt(stage, description),
        })
        .collect();

    Ok(SddPlan {
        constitution,
        speckit: detection,
        stages: planned,
    })
}

/// Human-readable next step after a failed stage (stderr + slash / specify init).
pub fn stage_failure_message(stage: &SddStagePlan, stderr_tail: &str, provider: &str) -> String {
    let tail = stderr_tail.trim();
    let next = format!(
        "Next: run `{}` in the worker, or `specify init --here --force --ignore-agent-tools --integration {provider}` if Spec-Kit skills are missing",
        stage.slash
    );
    if tail.is_empty() {
        format!("Spec-Kit stage `{}` failed. {next}", stage.stage)
    } else {
        format!("Spec-Kit stage `{}` failed: {tail}. {next}", stage.stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_sdd_without_speckit() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = prepare(tmp.path(), false, &[], "propose a backend").expect_err("sdd blocked");
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
        let plan = prepare(tmp.path(), true, &[], "propose a backend").expect("plan");
        assert_eq!(plan.constitution.status, ConstitutionStatus::Reused);
        assert_eq!(std::fs::read_to_string(tmp.path().join(".specify/memory/constitution.md")).unwrap(), "keep\n");
        assert_eq!(plan.stages.len(), 5);
        assert_eq!(plan.stages[0].stage, "specify");
        assert_eq!(plan.stages[0].slash, "/speckit.specify");
        assert!(plan.stages[0].prompt.contains("/speckit.specify"));
        assert_eq!(plan.stages[4].stage, "converge");
    }

    #[test]
    fn stage_failure_includes_stderr_and_next_command() {
        let stage = SddStagePlan {
            stage: "specify".to_string(),
            slash: "/speckit.specify".to_string(),
            skill: "speckit-specify".to_string(),
            prompt: String::new(),
        };
        let msg = stage_failure_message(&stage, "No such command 'specify'", "opencode");
        assert!(msg.contains("No such command 'specify'"));
        assert!(msg.contains("/speckit.specify"));
        assert!(msg.contains("specify init --here"));
    }
}
