//! Test-runner check for RDD FAST (spec §31).
//!
//! For phase 2 the only detected runner is Cargo (`Cargo.toml` present). The
//! command runs with a hard timeout and its output is captured to files so a
//! verbose runner cannot deadlock on a full pipe.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};

/// Outcome of the test check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestRunResult {
    /// `passed`, `failed` or `unavailable`.
    pub status: String,
    pub detail: String,
}

/// Detect the test runner for a working directory.
pub fn detect_runner(dir: &Path) -> Option<(&'static str, Vec<&'static str>)> {
    if dir.join("Cargo.toml").is_file() {
        Some(("cargo", vec!["test"]))
    } else {
        None
    }
}

/// Run the detected tests; never returns `passed` when the runner is missing or
/// fails (spec §36 policy: a required check must not pass silently).
pub fn run_tests(dir: &Path, timeout: Duration) -> NodkrayResult<TestRunResult> {
    let Some((program, args)) = detect_runner(dir) else {
        return Ok(TestRunResult {
            status: "unavailable".to_string(),
            detail: "no test runner detected (no Cargo.toml)".to_string(),
        });
    };

    let capture_dir = std::env::temp_dir().join(format!("nodkray-tests-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&capture_dir).map_err(|err| {
        NodkrayError::review("TEST_CAPTURE_FAILED", err.to_string())
    })?;
    let stdout_path = capture_dir.join("stdout.log");
    let stderr_path = capture_dir.join("stderr.log");

    let stdout_file = std::fs::File::create(&stdout_path)
        .map_err(|err| NodkrayError::review("TEST_CAPTURE_FAILED", err.to_string()))?;
    let stderr_file = std::fs::File::create(&stderr_path)
        .map_err(|err| NodkrayError::review("TEST_CAPTURE_FAILED", err.to_string()))?;

    let mut child = match Command::new(program)
        .args(&args)
        .current_dir(dir)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&capture_dir);
            return Ok(TestRunResult {
                status: "unavailable".to_string(),
                detail: format!("could not run {program}: {err}"),
            });
        }
    };

    let deadline = Instant::now() + timeout;
    let mut exit_status = None;
    let mut timed_out = false;
    loop {
        match child
            .try_wait()
            .map_err(|err| NodkrayError::review("TEST_RUN_FAILED", err.to_string()))?
        {
            Some(status) => {
                exit_status = Some(status);
                break;
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    let stdout = std::fs::read_to_string(&stdout_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&capture_dir);

    let (status, detail) = if timed_out {
        (
            "failed",
            format!(
                "{program} {} timed out after {}s",
                args.join(" "),
                timeout.as_secs()
            ),
        )
    } else if exit_status.map(|status| status.success()).unwrap_or(false) {
        (
            "passed",
            format!("{program} {} passed; {}", args.join(" "), tail(&stdout)),
        )
    } else {
        (
            "failed",
            format!(
                "{program} {} failed; {}{}",
                args.join(" "),
                tail(&stdout),
                tail(&stderr)
            ),
        )
    };

    Ok(TestRunResult {
        status: status.to_string(),
        detail,
    })
}

fn tail(text: &str) -> String {
    const MAX: usize = 600;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    let skip = trimmed.chars().count() - MAX;
    format!("…{}", trimmed.chars().skip(skip).collect::<String>())
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn missing_runner_is_unavailable_not_passed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = run_tests(tmp.path(), Duration::from_secs(5)).expect("run");
        assert_eq!(result.status, "unavailable");
    }

    #[test]
    fn cargo_project_is_detected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"\n")
            .expect("write");
        assert_eq!(detect_runner(tmp.path()).map(|(program, _)| program), Some("cargo"));
    }
}
