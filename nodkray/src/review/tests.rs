//! Test validator (spec §29). Detects a local test runner; never invents a pass.

use std::path::Path;
use std::process::Command;

use super::types::{CheckStatus, ReviewCheck};

pub fn check_tests(root: &Path) -> ReviewCheck {
    let Some((program, args)) = detect_runner(root) else {
        return ReviewCheck {
            id: "tests".into(),
            status: CheckStatus::Blocked,
            message: Some("no test runner detected; mandatory check unavailable".into()),
        };
    };

    let label = format!("{program} {}", args.join(" "));
    match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) if output.status.success() => ReviewCheck {
            id: "tests".into(),
            status: CheckStatus::Passed,
            message: Some(label),
        },
        Ok(output) => ReviewCheck {
            id: "tests".into(),
            status: CheckStatus::Failed,
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(err) => ReviewCheck {
            id: "tests".into(),
            status: CheckStatus::Blocked,
            message: Some(format!("failed to run tests: {err}")),
        },
    }
}

fn detect_runner(root: &Path) -> Option<(&'static str, Vec<&'static str>)> {
    if root.join("Cargo.toml").is_file() {
        return Some(("cargo", vec!["test", "--offline"]));
    }
    if root.join("package.json").is_file() {
        return Some(("npm", vec!["test", "--silent"]));
    }
    if root.join("pyproject.toml").is_file() || root.join("pytest.ini").is_file() {
        return Some(("pytest", vec!["-q"]));
    }
    None
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn missing_runner_is_blocked_not_passed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let check = check_tests(tmp.path());
        assert_eq!(check.status, CheckStatus::Blocked);
        assert_ne!(check.status, CheckStatus::Passed);
    }
}
