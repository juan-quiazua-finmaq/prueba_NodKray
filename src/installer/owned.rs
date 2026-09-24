//! Record what NodKray created versus what was already in the project.
//!
//! Uninstall uses this so MCP markers, Spec-Kit, Herdr and other pre-existing
//! tool configs are never deleted.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::NodkrayResult;

pub const FILENAME: &str = "owned.json";

/// Paths NodKray must never delete, even with `--project`.
pub const PROTECTED_MARKERS: &[&str] = &[
    ".serena",
    ".codegraph",
    ".sentrux",
    ".specify",
    ".herdr",
];

/// Manifest written under `<root>/.nodkray/owned.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OwnedManifest {
    /// `AGENTS.md` did not exist before this init.
    pub created_agents_md: bool,
    /// `.gitignore` did not exist before this init.
    pub created_gitignore: bool,
    /// Marker directories / files that were already present.
    pub preexisting: Vec<String>,
    /// Optional tools NodKray provisioned in this session (never uninstalled).
    pub provisioned: Vec<String>,
}

impl OwnedManifest {
    pub fn detect(project_root: &Path) -> Self {
        let mut preexisting = Vec::new();
        if project_root.join("AGENTS.md").is_file() {
            preexisting.push("AGENTS.md".to_string());
        }
        if project_root.join(".gitignore").is_file() {
            preexisting.push(".gitignore".to_string());
        }
        for marker in PROTECTED_MARKERS {
            if project_root.join(marker).exists() {
                preexisting.push((*marker).to_string());
            }
        }
        Self {
            created_agents_md: !preexisting.iter().any(|p| p == "AGENTS.md"),
            created_gitignore: !preexisting.iter().any(|p| p == ".gitignore"),
            preexisting,
            provisioned: Vec::new(),
        }
    }

    pub fn path(project_dir: &Path) -> std::path::PathBuf {
        project_dir.join(FILENAME)
    }

    pub fn load(project_dir: &Path) -> NodkrayResult<Option<Self>> {
        let path = Self::path(project_dir);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&text).unwrap_or_default()))
    }

    pub fn save(&self, project_dir: &Path) -> NodkrayResult<()> {
        std::fs::create_dir_all(project_dir)?;
        std::fs::write(Self::path(project_dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_preexisting(&self, name: &str) -> bool {
        self.preexisting.iter().any(|p| p == name)
    }
}

impl From<serde_json::Error> for crate::error::NodkrayError {
    fn from(err: serde_json::Error) -> Self {
        crate::error::NodkrayError::internal("JSON_ERROR", err.to_string())
    }
}

/// `true` when a path is a protected tool/MCP location that uninstall must skip.
pub fn is_protected(project_root: &Path, path: &Path) -> bool {
    for marker in PROTECTED_MARKERS {
        let protected = project_root.join(marker);
        if path == protected || path.starts_with(&protected) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_records_existing_mcp_markers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".serena")).expect("serena");
        std::fs::write(tmp.path().join("AGENTS.md"), "keep\n").expect("agents");
        let owned = OwnedManifest::detect(tmp.path());
        assert!(owned.is_preexisting(".serena"));
        assert!(owned.is_preexisting("AGENTS.md"));
        assert!(!owned.created_agents_md);
        assert!(!owned.is_preexisting(".codegraph"));
    }

    #[test]
    fn protected_paths_are_never_project_owned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        assert!(is_protected(root, &root.join(".specify").join("memory")));
        assert!(!is_protected(root, &root.join(".nodkray")));
    }
}
