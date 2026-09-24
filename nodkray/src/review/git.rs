//! Git diff validator (spec §29, §38).

use std::path::Path;
use std::process::Command;

use super::types::{CheckStatus, ReviewCheck};

pub fn check_diff(root: &Path) -> ReviewCheck {
    match Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--stat", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => ReviewCheck {
            id: "git-diff".into(),
            status: CheckStatus::Passed,
            message: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        },
        Ok(output) => ReviewCheck {
            id: "git-diff".into(),
            status: CheckStatus::Failed,
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(err) => ReviewCheck {
            id: "git-diff".into(),
            status: CheckStatus::Blocked,
            message: Some(format!("git is unavailable: {err}")),
        },
    }
}

/// Detect a merge conflict without resolving it.
pub fn detect_conflict(root: &Path) -> ReviewCheck {
    match Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-u"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let unmerged = String::from_utf8_lossy(&output.stdout);
            if unmerged.trim().is_empty() {
                ReviewCheck {
                    id: "git-conflict".into(),
                    status: CheckStatus::Passed,
                    message: None,
                }
            } else {
                ReviewCheck {
                    id: "git-conflict".into(),
                    status: CheckStatus::Blocked,
                    message: Some("unresolved conflict; NodKray will not auto-resolve".into()),
                }
            }
        }
        Ok(output) => ReviewCheck {
            id: "git-conflict".into(),
            status: CheckStatus::Failed,
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(err) => ReviewCheck {
            id: "git-conflict".into(),
            status: CheckStatus::Blocked,
            message: Some(err.to_string()),
        },
    }
}
