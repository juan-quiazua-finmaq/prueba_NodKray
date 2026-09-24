//! Project constitution detection and reuse (spec §24-§25, §68).

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};

/// Canonical Spec-Kit constitution path.
pub const CONSTITUTION_RELATIVE: &str = ".specify/memory/constitution.md";

/// Outcome of constitution detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstitutionStatus {
    Reused,
    Created,
    Absent,
}

/// Detection report. NodKray never overwrites an existing constitution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstitutionReport {
    pub path: String,
    pub status: ConstitutionStatus,
}

/// Resolve the constitution path for a project root.
pub fn constitution_path(project_root: &Path) -> PathBuf {
    project_root.join(CONSTITUTION_RELATIVE)
}

/// Detect an existing constitution without creating one.
pub fn detect(project_root: &Path) -> Option<PathBuf> {
    let path = constitution_path(project_root);
    path.is_file().then_some(path)
}

/// Reuse an existing constitution, or create it only when `create` is true.
///
/// Never overwrites, regenerates or duplicates an existing file.
pub fn ensure(project_root: &Path, create: bool) -> NodkrayResult<ConstitutionReport> {
    let path = constitution_path(project_root);
    if path.is_file() {
        return Ok(ConstitutionReport {
            path: path.display().to_string(),
            status: ConstitutionStatus::Reused,
        });
    }
    if !create {
        return Ok(ConstitutionReport {
            path: path.display().to_string(),
            status: ConstitutionStatus::Absent,
        });
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        return Err(NodkrayError::configuration(
            "CONSTITUTION_EXISTS",
            format!("refusing to overwrite {}", path.display()),
        ));
    }
    std::fs::write(&path, default_constitution())?;
    Ok(ConstitutionReport {
        path: path.display().to_string(),
        status: ConstitutionStatus::Created,
    })
}

fn default_constitution() -> &'static str {
    "# Project Constitution\n\nThis constitution is the reusable source of project rules for SDD.\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_and_never_overwrites() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = constitution_path(tmp.path());
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "original\n").expect("write");

        let detected = detect(tmp.path()).expect("detected");
        assert_eq!(detected, path);

        let report = ensure(tmp.path(), true).expect("ensure");
        assert_eq!(report.status, ConstitutionStatus::Reused);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "original\n");
    }

    #[test]
    fn create_only_when_requested() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let absent = ensure(tmp.path(), false).expect("absent");
        assert_eq!(absent.status, ConstitutionStatus::Absent);
        assert!(!constitution_path(tmp.path()).exists());

        let created = ensure(tmp.path(), true).expect("created");
        assert_eq!(created.status, ConstitutionStatus::Created);
        assert!(constitution_path(tmp.path()).is_file());
    }
}
