//! Lint validator used from RDD BALANCED/DEEP.

use std::path::Path;
use std::process::Command;

use super::types::{CheckStatus, ReviewCheck};

pub fn check_lint(root: &Path) -> ReviewCheck {
    let Some((program, args)) = detect_linter(root) else {
        return ReviewCheck {
            id: "lint".into(),
            status: CheckStatus::Skipped,
            message: Some("no linter detected".into()),
        };
    };
    match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) if output.status.success() => ReviewCheck {
            id: "lint".into(),
            status: CheckStatus::Passed,
            message: Some(program.to_string()),
        },
        Ok(output) => ReviewCheck {
            id: "lint".into(),
            status: CheckStatus::Failed,
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(err) => ReviewCheck {
            id: "lint".into(),
            status: CheckStatus::Skipped,
            message: Some(err.to_string()),
        },
    }
}

fn detect_linter(root: &Path) -> Option<(&'static str, Vec<&'static str>)> {
    if root.join("Cargo.toml").is_file() {
        return Some(("cargo", vec!["clippy", "--offline", "--", "-D", "warnings"]));
    }
    if root.join("package.json").is_file() {
        return Some(("npx", vec!["eslint", "."]));
    }
    None
}
