//! Spec-Kit adapter (spec §26-§28). NodKray coordinates; Spec-Kit executes SDD.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::agents::traits::ProcessSpec;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::tools::find_in_path;

use super::constitution::{self, ConstitutionReport};

/// Formal Spec-Kit stages. Optional quality gates are retained as first-class
/// values so the adapter never invents a parallel workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpecKitStage {
    Specify,
    Clarify,
    Plan,
    Checklist,
    Tasks,
    Analyze,
    Implement,
    Converge,
}

impl SpecKitStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Specify => "specify",
            Self::Clarify => "clarify",
            Self::Plan => "plan",
            Self::Checklist => "checklist",
            Self::Tasks => "tasks",
            Self::Analyze => "analyze",
            Self::Implement => "implement",
            Self::Converge => "converge",
        }
    }

    /// Agent slash command for this stage (Spec-Kit 1.0.x).
    pub fn slash_command(self) -> &'static str {
        match self {
            Self::Specify => "/speckit.specify",
            Self::Clarify => "/speckit.clarify",
            Self::Plan => "/speckit.plan",
            Self::Checklist => "/speckit.checklist",
            Self::Tasks => "/speckit.tasks",
            Self::Analyze => "/speckit.analyze",
            Self::Implement => "/speckit.implement",
            Self::Converge => "/speckit.converge",
        }
    }

    /// Agent skill name that accompanies the slash command.
    pub fn skill_name(self) -> &'static str {
        match self {
            Self::Specify => "speckit-specify",
            Self::Clarify => "speckit-clarify",
            Self::Plan => "speckit-plan",
            Self::Checklist => "speckit-checklist",
            Self::Tasks => "speckit-tasks",
            Self::Analyze => "speckit-analyze",
            Self::Implement => "speckit-implement",
            Self::Converge => "speckit-converge",
        }
    }

    /// Core cycle used when driving SDD without extra gates.
    pub fn core_cycle() -> &'static [SpecKitStage] {
        &[
            Self::Specify,
            Self::Plan,
            Self::Tasks,
            Self::Implement,
            Self::Converge,
        ]
    }
}

/// How Spec-Kit was located.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKitSource {
    Path,
    ProjectMarker,
    Missing,
}

/// Detection snapshot. Existing installs are reused, never overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpecKitDetection {
    pub installed: bool,
    pub source: SpecKitSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub project_marker: bool,
}

/// Adapter that only constructs Spec-Kit invocations.
#[derive(Debug, Clone)]
pub struct SpecKitAdapter {
    executable: PathBuf,
}

impl SpecKitAdapter {
    /// Locate an existing `specify` binary. Does not provision one.
    pub fn detect(project_root: Option<&Path>) -> SpecKitDetection {
        let executable = find_in_path("specify");
        let project_marker = project_root
            .map(|root| root.join(".specify").exists())
            .unwrap_or(false);
        let version = executable.as_ref().and_then(|path| probe_version(path));
        let installed = executable.is_some() || project_marker;
        let source = if executable.is_some() {
            SpecKitSource::Path
        } else if project_marker {
            SpecKitSource::ProjectMarker
        } else {
            SpecKitSource::Missing
        };
        SpecKitDetection {
            installed,
            source,
            executable,
            version,
            project_marker,
        }
    }

    /// Require a usable executable. SDD must not silently fall back to ST.
    pub fn locate(project_root: Option<&Path>) -> NodkrayResult<Self> {
        let detection = Self::detect(project_root);
        match detection.executable {
            Some(executable) => Ok(Self { executable }),
            None => Err(NodkrayError::dependency(
                "SPECKIT_NOT_FOUND",
                "Spec-Kit (`specify`) is required for SDD and was not found",
            )),
        }
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Prompt the configured worker to run one Spec-Kit **agent** stage.
    ///
    /// Spec-Kit 1.0.x does not implement `specify specify|plan|…` as CLI
    /// subcommands. Those names are slash commands (`/speckit.specify`, …).
    pub fn stage_prompt(stage: SpecKitStage, description: &str) -> String {
        format!(
            "Run Spec-Kit stage `{stage}` in this worktree.\n\
             Use the slash command `{slash}` (skill `{skill}`) to complete this stage for:\n\
             {description}\n\n\
             Stay inside project.root. Do not leave this repository.\n\
             If the Spec-Kit skills are missing, tell the user to run:\n\
             `specify init --here --force --ignore-agent-tools`\n\
             When the stage artifacts exist, return the Output Contract JSON.",
            stage = stage.as_str(),
            slash = stage.slash_command(),
            skill = stage.skill_name(),
            description = description.trim(),
        )
    }

    /// CLI helper only (`specify version`, `specify check`, `specify init`).
    /// Workflows must not treat this as a stage runner.
    pub fn command(
        &self,
        stage: SpecKitStage,
        project_root: &Path,
        extra: &[String],
    ) -> ProcessSpec {
        let _ = (stage, extra);
        ProcessSpec {
            program: self.executable.display().to_string(),
            args: vec!["version".to_string()],
            stdin: None,
            env: vec![(
                "SPECIFY_INIT_DIR".to_string(),
                project_root.display().to_string(),
            )],
        }
    }

    /// Detect or optionally create the project constitution.
    pub fn ensure_constitution(
        project_root: &Path,
        create: bool,
    ) -> NodkrayResult<ConstitutionReport> {
        constitution::ensure(project_root, create)
    }
}

fn probe_version(executable: &Path) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_cycle_matches_spec_kit() {
        let names: Vec<_> = SpecKitStage::core_cycle()
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            names,
            ["specify", "plan", "tasks", "implement", "converge"]
        );
    }

    #[test]
    fn command_is_cli_not_stage() {
        let adapter = SpecKitAdapter {
            executable: PathBuf::from("/usr/bin/specify"),
        };
        let spec = adapter.command(SpecKitStage::Plan, Path::new("/repo"), &[]);
        assert_eq!(spec.program, "/usr/bin/specify");
        assert_eq!(spec.args, ["version"]);
    }

    #[test]
    fn stage_prompt_uses_slash_commands() {
        let prompt = SpecKitAdapter::stage_prompt(SpecKitStage::Specify, "add a backend");
        assert!(prompt.contains("/speckit.specify"));
        assert!(prompt.contains("speckit-specify"));
        assert!(!prompt.contains("specify specify"));
    }

    #[test]
    fn locate_fails_when_missing() {
        if find_in_path("specify").is_some() {
            return;
        }
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = SpecKitAdapter::locate(Some(tmp.path())).expect_err("missing");
        assert_eq!(err.code(), "SPECKIT_NOT_FOUND");
        assert_eq!(err.category().as_str(), "dependency");
    }
}
