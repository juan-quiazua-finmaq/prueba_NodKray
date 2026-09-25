//! Execution backend registry (spec §151, §166).

use super::console::ConsoleBackend;
use super::herdr::HerdrBackend;
use super::traits::ExecutionBackend;
use crate::error::{NodkrayError, NodkrayResult};

/// Select a backend. If Herdr is requested but missing, optionally fall back.
pub fn select(name: &str, allow_console_fallback: bool) -> NodkrayResult<Box<dyn ExecutionBackend>> {
    match name.trim().to_ascii_lowercase().as_str() {
        "herdr" => {
            let herdr = HerdrBackend;
            if herdr.available() {
                Ok(Box::new(herdr))
            } else if allow_console_fallback {
                Ok(Box::new(ConsoleBackend))
            } else {
                Err(NodkrayError::dependency(
                    "HERDR_UNAVAILABLE",
                    "execution.backend is herdr but Herdr is not a usable process backend (`herdr run` missing)",
                ))
            }
        }
        "console" => Ok(Box::new(ConsoleBackend)),
        other => Err(NodkrayError::configuration(
            "UNKNOWN_EXECUTION_BACKEND",
            format!("unknown execution backend '{other}'"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herdr_falls_back_to_console_when_allowed() {
        if HerdrBackend.available() {
            assert_eq!(select("herdr", false).expect("herdr").name(), "herdr");
        } else {
            match select("herdr", false) {
                Ok(_) => panic!("herdr should be unavailable"),
                Err(err) => assert_eq!(err.code(), "HERDR_UNAVAILABLE"),
            }
            match select("herdr", true) {
                Ok(backend) => assert_eq!(backend.name(), "console"),
                Err(err) => panic!("fallback failed: {err}"),
            }
        }
    }

    #[test]
    fn herdr_without_run_contract_falls_back() {
        if crate::installer::probes::herdr_runner_usable() {
            return;
        }
        let backend = select("herdr", true).expect("console fallback");
        assert_eq!(backend.name(), "console");
    }
}
