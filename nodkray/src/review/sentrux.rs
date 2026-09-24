//! Optional Sentrux validator (spec §33-§34). Sentrux is not RDD.

use std::path::Path;
use std::process::Command;

use super::types::{CheckStatus, ReviewCheck};
use crate::installer::tools::find_in_path;

pub fn check_sentrux(root: &Path, enabled: bool, required: bool) -> ReviewCheck {
    if !enabled {
        return ReviewCheck {
            id: "sentrux".into(),
            status: CheckStatus::Skipped,
            message: Some("sentrux disabled".into()),
        };
    }
    let has_rules = root.join(".sentrux/rules.toml").is_file();
    let binary = find_in_path("sentrux");
    if binary.is_none() && !has_rules {
        return ReviewCheck {
            id: "sentrux".into(),
            status: if required {
                CheckStatus::Blocked
            } else {
                CheckStatus::Skipped
            },
            message: Some("sentrux not installed and .sentrux/rules.toml absent".into()),
        };
    }
    let Some(program) = binary else {
        return ReviewCheck {
            id: "sentrux".into(),
            status: if required {
                CheckStatus::Blocked
            } else {
                CheckStatus::Skipped
            },
            message: Some("sentrux binary not on PATH".into()),
        };
    };
    match Command::new(program).args(["check", "."]).current_dir(root).output() {
        Ok(output) if output.status.success() => ReviewCheck {
            id: "sentrux".into(),
            status: CheckStatus::Passed,
            message: None,
        },
        Ok(output) => ReviewCheck {
            id: "sentrux".into(),
            status: CheckStatus::Failed,
            message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(err) => ReviewCheck {
            id: "sentrux".into(),
            status: CheckStatus::Blocked,
            message: Some(err.to_string()),
        },
    }
}
