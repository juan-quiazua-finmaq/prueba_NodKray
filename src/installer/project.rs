//! Project/global installation (spec §64-§69, §97-§98).
//!
//! `init` is idempotent: existing files are detected and reported but never
//! rewritten, and existing rules (CONSTITUTION.md, AGENTS.md, .specify/, …) are
//! only listed, never modified.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{default_value, load_effective, write_value, Config, ConfigPaths};
use crate::core::project::find_project_root;
use crate::error::NodkrayResult;
use crate::installer::{agents_md, gitignore, owned, skills};
use crate::memory::heal::{self, HealReport};

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    pub agents_md: bool,
    pub gitignore: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<HealReport>,
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

    let manifest = owned::OwnedManifest::load(&project_dir)?
        .unwrap_or_else(|| owned::OwnedManifest::detect(&root));
    let skills_written = skills::install_into(&root)?;
    if !skills_written.is_empty() {
        created.push("skills/nodkray".to_string());
    }
    let agents_md_changed = agents_md::ensure_block(&root)?;
    if agents_md_changed {
        created.push("AGENTS.md".to_string());
    }
    let gitignore_changed = gitignore::ensure_block(&root, manifest.created_agents_md)?;
    if gitignore_changed {
        created.push(".gitignore".to_string());
    }
    let _ = manifest.save(&project_dir);

    let memory = match load_effective(paths, Some(&root)) {
        Ok(config) => {
            let db_path = paths.resolve_memory_path(&config.memory.path);
            heal::heal(&db_path).ok().map(|(_, report)| report)
        }
        Err(_) => None,
    };

    Ok(ProjectInitReport {
        root: root.display().to_string(),
        config_path: config_path.display().to_string(),
        git: found.git_root.is_some(),
        created,
        existing,
        rules: detect_rules(&root),
        skills: skills_written,
        agents_md: agents_md_changed,
        gitignore: gitignore_changed,
        memory,
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
        assert!(
            !second.created.iter().any(|c| c == ".nodkray/config.yaml"),
            "second init must not rewrite config: {:?}",
            second.created
        );
        assert!(second.created.is_empty(), "second init created {:?}", second.created);
        assert!(second
            .existing
            .iter()
            .any(|e| e == ".nodkray/config.yaml"));
    }

    #[test]
    fn init_project_appends_agents_md_without_losing_user_text() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let agents_md = repo.join("AGENTS.md");
        std::fs::write(&agents_md, "original bytes\n").expect("write");

        let report = init_project(&paths, &repo).expect("init");
        assert!(report.rules.iter().any(|r| r.name == "AGENTS.md"));
        let after = std::fs::read_to_string(&agents_md).expect("read");
        assert!(after.contains("original bytes"));
        assert!(after.contains(crate::installer::agents_md::BEGIN));
        assert!(repo.join(".gitignore").is_file());
        assert!(repo.join("skills").join("nodkray").join("test.md").is_file());
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
