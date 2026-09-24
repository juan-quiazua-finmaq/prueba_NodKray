//! Git helpers for review and merge (spec §38-§39).
//!
//! All commands are plain `git -C <dir>` invocations; NodKray never resolves a
//! conflict silently — a conflicting merge is aborted and reported.

use std::path::Path;
use std::process::{Command, Output};

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};

fn run_git(dir: &Path, args: &[&str]) -> NodkrayResult<Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|err| NodkrayError::git("GIT_UNAVAILABLE", err.to_string()))
}

fn git_stdout(dir: &Path, args: &[&str]) -> NodkrayResult<String> {
    let output = run_git(dir, args)?;
    if !output.status.success() {
        return Err(NodkrayError::git(
            "GIT_COMMAND_FAILED",
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_succeeds(dir: &Path, args: &[&str]) -> NodkrayResult<bool> {
    Ok(run_git(dir, args)?.status.success())
}

/// Files that differ from `base`, including untracked files. Sorted, deduped.
pub fn changed_files(dir: &Path, base: &str) -> NodkrayResult<Vec<String>> {
    let mut files: Vec<String> = Vec::new();
    if let Ok(text) = git_stdout(dir, &["diff", "--name-only", base]) {
        files.extend(text.lines().map(str::to_string));
    }
    if let Ok(text) = git_stdout(dir, &["ls-files", "--others", "--exclude-standard"]) {
        files.extend(text.lines().map(str::to_string));
    }
    files.retain(|line| !line.trim().is_empty());
    files.sort();
    files.dedup();
    Ok(files)
}

/// `git diff --stat` summary against `base`.
pub fn diff_stat(dir: &Path, base: &str) -> NodkrayResult<String> {
    Ok(git_stdout(dir, &["diff", "--stat", base]).unwrap_or_default())
}

/// Current branch name, if the repository is on one.
pub fn current_branch(dir: &Path) -> NodkrayResult<Option<String>> {
    match git_stdout(dir, &["symbolic-ref", "--short", "HEAD"]) {
        Ok(text) => {
            let branch = text.trim().to_string();
            Ok((!branch.is_empty()).then_some(branch))
        }
        Err(_) => Ok(None),
    }
}

/// Stage and commit everything. Returns `true` when a commit was created.
pub fn commit_all(dir: &Path, message: &str) -> NodkrayResult<bool> {
    if !git_succeeds(dir, &["add", "-A"])? {
        return Err(NodkrayError::git(
            "GIT_ADD_FAILED",
            format!("git add failed in {}", dir.display()),
        ));
    }
    if git_succeeds(dir, &["diff", "--cached", "--quiet"])? {
        return Ok(false);
    }
    let output = run_git(dir, &["commit", "-m", message])?;
    if !output.status.success() {
        return Err(NodkrayError::git(
            "GIT_COMMIT_FAILED",
            format!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(true)
}

/// True when the working tree has no changes (tracked or untracked).
pub fn is_clean(dir: &Path) -> NodkrayResult<bool> {
    Ok(git_stdout(dir, &["status", "--porcelain"])
        .map(|text| text.trim().is_empty())
        .unwrap_or(false))
}

/// Outcome of a merge attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    Merged,
    Conflict,
    Failed,
}

/// Result of merging a worker branch into the main working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeOutcome {
    pub status: MergeStatus,
    pub detail: String,
}

/// Merge `branch` into the current branch of `root`.
///
/// On conflict the merge is aborted (leaving git in a clean state) and
/// [`MergeStatus::Conflict`] is returned; NodKray never resolves it (spec §39).
pub fn merge_branch(root: &Path, branch: &str) -> NodkrayResult<MergeOutcome> {
    let output = run_git(root, &["merge", "--no-ff", "--no-edit", branch])?;
    if output.status.success() {
        return Ok(MergeOutcome {
            status: MergeStatus::Merged,
            detail: format!("merged {branch}"),
        });
    }

    let unmerged = git_stdout(root, &["ls-files", "-u"])
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    // Always abort so git does not stay in "merge in progress".
    let abort = run_git(root, &["merge", "--abort"]);
    let aborted = abort.map(|out| out.status.success()).unwrap_or(false);

    if unmerged {
        return Ok(MergeOutcome {
            status: MergeStatus::Conflict,
            detail: format!("merge conflict on {branch}; aborted={aborted}; {stderr}"),
        });
    }

    Ok(MergeOutcome {
        status: MergeStatus::Failed,
        detail: format!("merge failed on {branch}; aborted={aborted}; {stderr}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo(dir: &Path) {
        git(dir, &["init", "-q"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        std::fs::write(dir.join("file.txt"), "base\n").expect("write");
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", "base"]);
    }

    #[test]
    fn merge_of_diverging_branch_conflicts_and_aborts_clean() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        init_repo(&repo);

        // Diverging branch that edits the same line.
        git(&repo, &["checkout", "-q", "-b", "nodkray/other"]);
        std::fs::write(repo.join("file.txt"), "other\n").expect("write");
        git(&repo, &["commit", "-aqm", "other"]);

        // Main also advances.
        git(&repo, &["checkout", "-q", "master"]);
        std::fs::write(repo.join("file.txt"), "main\n").expect("write");
        git(&repo, &["commit", "-aqm", "main"]);

        let outcome = merge_branch(&repo, "nodkray/other").expect("merge");
        assert_eq!(outcome.status, MergeStatus::Conflict);
        // Git must be left clean with no merge in progress.
        assert!(is_clean(&repo).expect("clean"));
        assert!(!repo.join(".git").join("MERGE_HEAD").exists());
    }
}
