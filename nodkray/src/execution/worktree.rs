//! Parallel worker isolation via git worktrees (spec §16, §118).

use std::path::{Path, PathBuf};

use crate::error::{NodkrayError, NodkrayResult};

/// `.nodkray/worktrees/<worker-id>`
pub fn worktree_path(project_root: &Path, worker_id: &str) -> PathBuf {
    project_root
        .join(".nodkray")
        .join("worktrees")
        .join(sanitize_worker_id(worker_id))
}

fn sanitize_worker_id(worker_id: &str) -> String {
    worker_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

/// Create an isolated git worktree. The worker must never share another worker's path.
pub fn create_worktree(
    project_root: &Path,
    worker_id: &str,
    branch: Option<&str>,
) -> NodkrayResult<PathBuf> {
    let path = worktree_path(project_root, worker_id);
    if path.exists() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(project_root).arg("worktree").arg("add");
    if let Some(branch) = branch {
        command.arg("-b").arg(branch);
    }
    command.arg(&path);

    let output = command.output().map_err(|err| {
        NodkrayError::git("WORKTREE_SPAWN_FAILED", err.to_string())
    })?;
    if !output.status.success() {
        return Err(NodkrayError::git(
            "WORKTREE_CREATE_FAILED",
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(path)
}

/// True when `candidate` is inside `worker_id`'s worktree and not another worker's.
pub fn owns_path(project_root: &Path, worker_id: &str, candidate: &Path) -> bool {
    let own = worktree_path(project_root, worker_id);
    candidate.starts_with(&own)
}

pub fn assert_isolated(
    project_root: &Path,
    writer: &str,
    other: &str,
    candidate: &Path,
) -> NodkrayResult<()> {
    if owns_path(project_root, other, candidate) && writer != other {
        return Err(NodkrayError::permission(
            "WORKTREE_ISOLATION_VIOLATION",
            format!("{writer} must not write into {other}'s worktree"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workers_do_not_share_paths() {
        let root = Path::new("/repo");
        let a = worktree_path(root, "worker-a");
        let b = worktree_path(root, "worker-b");
        assert_ne!(a, b);
        assert!(owns_path(root, "worker-a", &a.join("src/lib.rs")));
        assert!(!owns_path(root, "worker-a", &b.join("src/lib.rs")));
        assert!(assert_isolated(root, "worker-a", "worker-b", &b.join("x")).is_err());
        assert!(assert_isolated(root, "worker-a", "worker-b", &a.join("x")).is_ok());
    }

    #[test]
    fn creates_git_worktree_in_temp_repo() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path();
        run(repo, &["init", "-b", "main"]);
        std::fs::write(repo.join("README.md"), "ok\n").expect("write");
        run(repo, &["add", "."]);
        run(repo, &["-c", "user.email=test@example.com", "-c", "user.name=Test", "commit", "-m", "init"]);

        let path = create_worktree(repo, "backend-01", Some("nk/backend-01")).expect("worktree");
        assert!(path.starts_with(repo.join(".nodkray/worktrees/backend-01")));
        assert!(path.exists());
        let again = create_worktree(repo, "backend-01", None).expect("idempotent");
        assert_eq!(again, path);
    }

    fn run(repo: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed");
    }
}
