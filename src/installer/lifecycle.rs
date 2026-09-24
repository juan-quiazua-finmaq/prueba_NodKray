//! Binary update and uninstall. Never reverses MCP or third-party tool installs.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::config::{load_effective, Config, ConfigPaths};
use crate::core::project::find_project_root;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::{agents_md, gitignore, owned, skills};
use crate::memory::heal::{self, HealReport};

const PROTECTED_NOTE: &str =
    "left MCP servers, Spec-Kit, Herdr, and any pre-existing tool configs untouched";

/// How to resolve the GitHub `owner/name` used by `update`.
pub fn resolve_repository(config: Option<&Config>) -> String {
    if let Ok(repo) = std::env::var("NODKRAY_REPO") {
        if !repo.is_empty() && repo != "__BAKE_REPO__" && repo != "__REPO__" {
            return repo;
        }
    }
    if let Some(repo) = config.and_then(|c| c.update.repository.clone()) {
        if !repo.is_empty() {
            return repo;
        }
    }
    if let Some(repo) = option_env!("NODKRAY_REPO") {
        if !repo.is_empty() {
            return repo.to_string();
        }
    }
    String::new()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateReport {
    pub repository: String,
    pub version: String,
    pub installed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<HealReport>,
}

pub fn update_binary(paths: &ConfigPaths, cwd: &Path) -> NodkrayResult<UpdateReport> {
    let root = find_project_root(cwd);
    let config = load_effective(paths, Some(&root.root)).ok();
    let repository = resolve_repository(config.as_ref());
    if repository.is_empty() {
        return Err(NodkrayError::configuration(
            "UPDATE_REPO_MISSING",
            "set NODKRAY_REPO=owner/name or config update.repository",
        ));
    }
    let version = std::env::var("NODKRAY_VERSION").unwrap_or_else(|_| "latest".to_string());
    let dest = std::env::current_exe().map_err(|err| {
        NodkrayError::internal("UPDATE_EXE_ERROR", format!("cannot locate current binary: {err}"))
    })?;

    let memory = heal_memory(paths, config.as_ref());

    let asset = release_asset_name();
    let url = if version == "latest" {
        format!("https://github.com/{repository}/releases/latest/download/{asset}")
    } else {
        format!("https://github.com/{repository}/releases/download/{version}/{asset}")
    };

    let tmp = tempfile_dir()?;
    download(&url, &tmp.join(&asset))?;
    let extracted = extract_binary(&tmp, &asset)?;
    replace_binary(&extracted, &dest)?;

    Ok(UpdateReport {
        repository,
        version,
        installed: dest.display().to_string(),
        memory,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct UninstallReport {
    pub removed: Vec<String>,
    pub skipped: Vec<String>,
    pub note: String,
}

/// Remove NodKray's own files. MCP / Spec-Kit / Herdr / pre-existing markers stay.
pub fn uninstall(
    paths: &ConfigPaths,
    cwd: &Path,
    include_project: bool,
) -> NodkrayResult<UninstallReport> {
    let mut report = UninstallReport {
        note: PROTECTED_NOTE.to_string(),
        ..UninstallReport::default()
    };

    if include_project {
        uninstall_project(paths, cwd, &mut report)?;
    }

    remove_path(&paths.config_home, &mut report);
    remove_path(&paths.data_home, &mut report);

    for candidate in install_binary_candidates() {
        if candidate.exists() && is_safe_to_remove_binary(&candidate) {
            remove_path(&candidate, &mut report);
        }
    }

    for marker in owned::PROTECTED_MARKERS {
        report
            .skipped
            .push(format!("{marker} (pre-existing or third-party; never removed)"));
    }

    Ok(report)
}

fn uninstall_project(
    paths: &ConfigPaths,
    cwd: &Path,
    report: &mut UninstallReport,
) -> NodkrayResult<()> {
    let root = find_project_root(cwd).root;
    // Older trees without a manifest: treat every protected marker that exists
    // as pre-existing so uninstall never deletes MCP/tool configs.
    let manifest = owned::OwnedManifest::load(&paths.project_dir(&root))?
        .unwrap_or_else(|| owned::OwnedManifest::detect(&root));

    let _ = agents_md::remove_block(&root);
    if manifest.created_agents_md {
        let agents = root.join("AGENTS.md");
        if agents.is_file() {
            let text = std::fs::read_to_string(&agents).unwrap_or_default();
            if text.trim().is_empty() {
                remove_path(&agents, report);
            } else {
                report
                    .skipped
                    .push("AGENTS.md (still has user content)".to_string());
            }
        }
    } else {
        report
            .skipped
            .push("AGENTS.md (existed before NodKray; block stripped only)".to_string());
    }

    let _ = gitignore::remove_block(&root);
    if manifest.created_gitignore {
        let gi = root.join(".gitignore");
        if gi.is_file() {
            let text = std::fs::read_to_string(&gi).unwrap_or_default();
            if text.trim().is_empty() {
                remove_path(&gi, report);
            }
        }
    } else {
        report
            .skipped
            .push(".gitignore (existed before NodKray; NodKray block stripped only)".to_string());
    }

    let skills_dir = skills::skills_dir(&root);
    if skills_dir.is_dir() {
        remove_path(&skills_dir, report);
        let parent = root.join("skills");
        if parent.is_dir() && dir_is_empty(&parent) {
            remove_path(&parent, report);
        }
    }

    for marker in owned::PROTECTED_MARKERS {
        let path = root.join(marker);
        if path.exists() {
            report.skipped.push(format!(
                "{} (MCP/tool config; uninstall never deletes this)",
                path.display()
            ));
        }
    }

    let project_dir = paths.project_dir(&root);
    if project_dir.is_dir() {
        remove_path(&project_dir, report);
    }
    Ok(())
}

fn heal_memory(paths: &ConfigPaths, config: Option<&Config>) -> Option<HealReport> {
    let configured = config
        .map(|c| c.memory.path.as_str())
        .unwrap_or(crate::config::schema::DEFAULT_MEMORY_PATH);
    let db_path = paths.resolve_memory_path(configured);
    match heal::heal(&db_path) {
        Ok((_, report)) => Some(report),
        Err(err) => Some(HealReport {
            path: db_path.display().to_string(),
            opened: false,
            recreated: false,
            backup: None,
            schema_version: None,
            message: err.to_string(),
        }),
    }
}

fn release_asset_name() -> String {
    let os = match std::env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => other,
    };
    if cfg!(windows) {
        format!("nodkray-{arch}-{os}.zip")
    } else {
        format!("nodkray-{arch}-{os}.tar.gz")
    }
}

fn download(url: &str, dest: &Path) -> NodkrayResult<()> {
    let status = if find_cmd("curl") {
        Command::new("curl")
            .args(["-fsSL", url, "-o"])
            .arg(dest)
            .status()
    } else if find_cmd("wget") {
        Command::new("wget").args(["-qO"]).arg(dest).arg(url).status()
    } else {
        return Err(NodkrayError::dependency(
            "UPDATE_DOWNLOADER_MISSING",
            "need curl or wget to download a release",
        ));
    }
    .map_err(|err| NodkrayError::network("UPDATE_DOWNLOAD_ERROR", err.to_string()))?;
    if !status.success() {
        return Err(NodkrayError::network(
            "UPDATE_DOWNLOAD_ERROR",
            format!("failed to download {url}"),
        ));
    }
    Ok(())
}

fn extract_binary(tmp: &Path, asset: &str) -> NodkrayResult<PathBuf> {
    let archive = tmp.join(asset);
    if asset.ends_with(".zip") {
        let status = Command::new("tar")
            .args(["-xf"])
            .arg(&archive)
            .current_dir(tmp)
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            // fall through
        } else if cfg!(windows) {
            let _ = Command::new("powershell")
                .args([
                    "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive.display(),
                        tmp.display()
                    ),
                ])
                .status();
        }
    } else {
        let status = Command::new("tar")
            .args(["-xzf"])
            .arg(&archive)
            .current_dir(tmp)
            .status()
            .map_err(|err| NodkrayError::internal("UPDATE_EXTRACT_ERROR", err.to_string()))?;
        if !status.success() {
            return Err(NodkrayError::internal(
                "UPDATE_EXTRACT_ERROR",
                "failed to extract release archive",
            ));
        }
    }

    let unix = tmp.join("nodkray");
    if unix.is_file() {
        return Ok(unix);
    }
    let windows = tmp.join("nodkray.exe");
    if windows.is_file() {
        return Ok(windows);
    }
    Err(NodkrayError::internal(
        "UPDATE_EXTRACT_ERROR",
        "archive did not contain nodkray",
    ))
}

fn replace_binary(src: &Path, dest: &Path) -> NodkrayResult<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staged = dest.with_extension("new");
    std::fs::copy(src, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&staged)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&staged, perms)?;
    }
    if dest.exists() {
        let old = dest.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(dest, &old)?;
        if let Err(err) = std::fs::rename(&staged, dest) {
            let _ = std::fs::rename(&old, dest);
            return Err(err.into());
        }
        let _ = std::fs::remove_file(&old);
    } else {
        std::fs::rename(&staged, dest)?;
    }
    Ok(())
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "nodkray.exe"
    } else {
        "nodkray"
    }
}

fn install_binary_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let default = default_install_path();
    if is_safe_to_remove_binary(&default) {
        out.push(default);
    }
    if let Ok(prefix) = std::env::var("NODKRAY_PREFIX") {
        if !prefix.is_empty() {
            let prefixed = PathBuf::from(prefix).join("bin").join(exe_name());
            if is_safe_to_remove_binary(&prefixed) {
                out.push(prefixed);
            }
        }
    }
    out
}

/// Only delete binaries that live in an install prefix, never Cargo test artifacts.
fn is_safe_to_remove_binary(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if text.contains("/target/")
        || text.contains("\\target\\")
        || text.contains("cargo-target")
        || text.contains("/deps/")
    {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == "nodkray" || name == "nodkray.exe"
}

fn default_install_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("nodkray").join("bin").join("nodkray.exe")
    } else {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".local").join("bin").join("nodkray")
    }
}

fn remove_path(path: &Path, report: &mut UninstallReport) {
    if !path.exists() {
        return;
    }
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match result {
        Ok(()) => report.removed.push(path.display().to_string()),
        Err(err) => report
            .skipped
            .push(format!("{} ({err})", path.display())),
    }
}

fn dir_is_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false)
}

fn tempfile_dir() -> NodkrayResult<PathBuf> {
    let dir = std::env::temp_dir().join(format!("nodkray-update-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn find_cmd(name: &str) -> bool {
    crate::installer::tools::find_in_path(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstall_project_preserves_preexisting_mcp_markers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("cfg"), tmp.path().join("data"));
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).expect("git");
        std::fs::create_dir_all(repo.join(".serena")).expect("serena");
        std::fs::write(repo.join(".serena").join("project.yml"), "keep\n").expect("mcp cfg");
        std::fs::write(repo.join("AGENTS.md"), "# Project\n\nKeep me.\n").expect("agents");
        crate::installer::project::init_project(&paths, &repo).expect("init");

        let report = uninstall(&paths, &repo, true).expect("uninstall");
        assert!(repo.join(".serena").join("project.yml").is_file());
        assert!(repo.join("AGENTS.md").is_file());
        let agents = std::fs::read_to_string(repo.join("AGENTS.md")).expect("read");
        assert!(agents.contains("Keep me."));
        assert!(!agents.contains(agents_md::BEGIN));
        assert!(!repo.join(".nodkray").exists());
        assert!(report.skipped.iter().any(|s| s.contains(".serena")));
    }

    #[test]
    fn resolve_repository_prefers_config() {
        let mut config = crate::config::Config::default();
        config.update.repository = Some("acme/NodKray".to_string());
        assert_eq!(resolve_repository(Some(&config)), "acme/NodKray");
    }
}
