//! Project discovery and context (spec §159).
//!
//! From the current working directory NodKray walks upwards looking for the
//! nearest repository root (a directory containing `.git`), falling back to the
//! nearest `.nodkray` marker and finally to the cwd itself.

use std::path::{Path, PathBuf};

use crate::config::{load_effective, Config, ConfigPaths};
use crate::error::NodkrayResult;

/// Project-local NodKray directory.
pub const PROJECT_DIR: &str = ".nodkray";
/// Git metadata directory (or file, in linked worktrees).
pub const GIT_DIR: &str = ".git";

/// Result of walking the filesystem upwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    /// Effective project root used to resolve `.nodkray/`.
    pub root: PathBuf,
    /// Nearest ancestor that looks like a git repository, when any.
    pub git_root: Option<PathBuf>,
}

/// A resolved project together with its effective configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub git_root: Option<PathBuf>,
    pub config: Config,
}

impl ProjectContext {
    /// Discover the project from `cwd` and load its effective config.
    pub fn discover(cwd: &Path, paths: &ConfigPaths) -> NodkrayResult<Self> {
        let found = find_project_root(cwd);
        let config = load_effective(paths, Some(&found.root))?;
        Ok(Self {
            root: found.root,
            git_root: found.git_root,
            config,
        })
    }
}

/// Walk upwards from `cwd` to locate the project root.
pub fn find_project_root(cwd: &Path) -> ProjectRoot {
    let start = cwd
        .canonicalize()
        .unwrap_or_else(|_| cwd.to_path_buf());

    let mut git_root: Option<PathBuf> = None;
    let mut nodkray_root: Option<PathBuf> = None;

    for ancestor in start.ancestors() {
        if git_root.is_none() && ancestor.join(GIT_DIR).exists() {
            git_root = Some(ancestor.to_path_buf());
        }
        if nodkray_root.is_none()
            && ancestor.join(PROJECT_DIR).join("config.yaml").exists()
        {
            nodkray_root = Some(ancestor.to_path_buf());
        }
        if git_root.is_some() {
            break;
        }
    }

    let root = git_root
        .clone()
        .or(nodkray_root)
        .unwrap_or_else(|| start.clone());

    ProjectRoot { root, git_root }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nearest_git_root_from_nested_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        let nested = repo.join("a").join("b");
        std::fs::create_dir_all(repo.join(".git")).expect("git dir");
        std::fs::create_dir_all(&nested).expect("nested");

        let found = find_project_root(&nested);
        assert_eq!(found.git_root.as_deref(), Some(repo.as_path()));
        assert_eq!(found.root, repo);
    }

    #[test]
    fn falls_back_to_cwd_without_markers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let found = find_project_root(tmp.path());
        assert!(found.git_root.is_none());
        assert_eq!(
            found.root,
            tmp.path().canonicalize().expect("canonicalize")
        );
    }

    #[test]
    fn uses_nodkray_marker_when_no_git() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join(".nodkray")).expect("nodkray dir");
        std::fs::write(project.join(".nodkray").join("config.yaml"), "version: 1\n")
            .expect("config");

        let found = find_project_root(&project);
        assert_eq!(found.root, project);
        assert!(found.git_root.is_none());
    }
}
