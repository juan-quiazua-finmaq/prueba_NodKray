//! Project/global installation (spec §64-§69, §97-§98).
//!
//! `init` is idempotent: existing files are detected and reported but never
//! rewritten, and existing rules (CONSTITUTION.md, AGENTS.md, .specify/, …) are
//! only listed, never modified.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{default_value, write_value, Config, ConfigPaths};
use crate::core::project::find_project_root;
use crate::error::NodkrayResult;

/// Rule/context files NodKray detects but never touches (spec §66).
pub const RULE_CANDIDATES: &[&str] = &[
    "CONSTITUTION.md",
    "CONSTITUTION.MD",
    ".specify/memory/constitution.md",
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    "AGENTS.local.md",
    ".sentrux/rules.toml",
];

/// A detected local rule file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleDetection {
    pub name: String,
    pub path: String,
}

/// Detect local rule files relative to `root`.
pub fn detect_rules(root: &Path) -> Vec<RuleDetection> {
    RULE_CANDIDATES
        .iter()
        .filter_map(|candidate| {
            let path = root.join(candidate);
            path.is_file().then(|| RuleDetection {
                name: (*candidate).to_string(),
                path: path.display().to_string(),
            })
        })
        .collect()
}

/// Result of `init --global`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GlobalInitReport {
    pub path: String,
    pub created: bool,
}

/// Create `~/.config/nodkray/config.yaml` if missing.
pub fn init_global(paths: &ConfigPaths) -> NodkrayResult<GlobalInitReport> {
    let path = paths.global_config_file();
    if path.exists() {
        return Ok(GlobalInitReport {
            path: path.display().to_string(),
            created: false,
        });
    }
    write_value(&path, &default_value())?;
    Ok(GlobalInitReport {
        path: path.display().to_string(),
        created: true,
    })
}

/// Result of `init --project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectInitReport {
    pub root: String,
    pub config_path: String,
    pub git: bool,
    pub created: Vec<String>,
    pub existing: Vec<String>,
    pub rules: Vec<RuleDetection>,
}

/// Create `<root>/.nodkray/{config.yaml,tasks/,worktrees/}` without touching
/// pre-existing files.
pub fn init_project(paths: &ConfigPaths, cwd: &Path) -> NodkrayResult<ProjectInitReport> {
    let found = find_project_root(cwd);
    let root = found.root;
    let project_dir = paths.project_dir(&root);

    let mut created = Vec::new();
    let mut existing = Vec::new();

    for (label, dir) in [
        (".nodkray", project_dir.clone()),
        (".nodkray/tasks", project_dir.join("tasks")),
        (".nodkray/worktrees", project_dir.join("worktrees")),
    ] {
        if dir.is_dir() {
            existing.push(label.to_string());
        } else {
            std::fs::create_dir_all(&dir).map_err(|err| {
                crate::error::NodkrayError::configuration(
                    "PROJECT_INIT_ERROR",
                    format!("could not create {}: {}", dir.display(), err),
                )
            })?;
            created.push(label.to_string());
        }
    }

    let config_path = paths.project_config_file(&root);
    if config_path.exists() {
        existing.push(".nodkray/config.yaml".to_string());
    } else {
        let mut config = Config::default();
        config.project.name = project_name(&root);
        let value = serde_yaml::to_value(&config).map_err(|err| {
            crate::error::NodkrayError::configuration("CONFIG_SERIALIZE_ERROR", err.to_string())
        })?;
        write_value(&config_path, &value)?;
        created.push(".nodkray/config.yaml".to_string());
    }

    Ok(ProjectInitReport {
        root: root.display().to_string(),
        config_path: config_path.display().to_string(),
        git: found.git_root.is_some(),
        created,
        existing,
        rules: detect_rules(&root),
    })
}

/// Derive a project name from the root directory name.
pub fn project_name(root: &Path) -> Option<String> {
    root.file_name().map(|name| name.to_string_lossy().into_owned())
}

/// Resolve the `.nodkray` directory for a root.
pub fn project_dir_for(root: &Path) -> PathBuf {
    root.join(".nodkray")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_project_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).expect("git");

        let first = init_project(&paths, &repo).expect("first init");
        assert!(first.created.iter().any(|c| c == ".nodkray/config.yaml"));
        let second = init_project(&paths, &repo).expect("second init");
        assert!(second.created.is_empty());
        assert!(second
            .existing
            .iter()
            .any(|e| e == ".nodkray/config.yaml"));
    }

    #[test]
    fn init_project_lists_but_does_not_touch_rules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let agents_md = repo.join("AGENTS.md");
        std::fs::write(&agents_md, "original bytes\n").expect("write");

        let report = init_project(&paths, &repo).expect("init");
        assert!(report.rules.iter().any(|r| r.name == "AGENTS.md"));
        let after = std::fs::read_to_string(&agents_md).expect("read");
        assert_eq!(after, "original bytes\n");
    }

    #[test]
    fn init_global_creates_then_preserves() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));

        let first = init_global(&paths).expect("first");
        assert!(first.created);
        std::fs::write(paths.global_config_file(), "version: 1\nsecurity:\n  yolo: true\n")
            .expect("overwrite");
        let second = init_global(&paths).expect("second");
        assert!(!second.created);
        let text = std::fs::read_to_string(paths.global_config_file()).expect("read");
        assert!(text.contains("yolo: true"));
    }
}
