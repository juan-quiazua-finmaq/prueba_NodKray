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

    /// Build the process spec for one Spec-Kit stage. Workflows never shell out
    /// to `specify` directly; they go through this adapter.
    pub fn command(
        &self,
        stage: SpecKitStage,
        project_root: &Path,
        extra: &[String],
    ) -> ProcessSpec {
        let mut args = vec![stage.as_str().to_string()];
        args.extend(extra.iter().cloned());
        let _ = project_root;
        ProcessSpec {
            program: self.executable.display().to_string(),
            args,
            stdin: None,
            env: Vec::new(),
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
    fn command_goes_through_adapter() {
        let adapter = SpecKitAdapter {
            executable: PathBuf::from("/usr/bin/specify"),
        };
        let spec = adapter.command(SpecKitStage::Plan, Path::new("/repo"), &[]);
        assert_eq!(spec.program, "/usr/bin/specify");
        assert_eq!(spec.args, ["plan"]);
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
