//! Local rule / constitution validator (spec §66, §84).

use std::path::Path;

use super::types::{CheckStatus, ReviewCheck};
use crate::installer::project::{detect_rules, RULE_CANDIDATES};
use crate::spec::constitution;

pub fn check_rules(root: &Path) -> ReviewCheck {
    let detected = detect_rules(root);
    if detected.is_empty() {
        return ReviewCheck {
            id: "rules".into(),
            status: CheckStatus::Passed,
            message: Some("no local rules detected".into()),
        };
    }
    let missing: Vec<_> = RULE_CANDIDATES
        .iter()
        .filter(|candidate| {
            let path = root.join(candidate);
            // Only flag a listed file that exists but is empty — presence is enough.
            path.is_file()
                && std::fs::read_to_string(path)
                    .map(|t| t.trim().is_empty())
                    .unwrap_or(false)
        })
        .copied()
        .collect();
    if missing.is_empty() {
        ReviewCheck {
            id: "rules".into(),
            status: CheckStatus::Passed,
            message: Some(
                detected
                    .iter()
                    .map(|r| r.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        }
    } else {
        ReviewCheck {
            id: "rules".into(),
            status: CheckStatus::Failed,
            message: Some(format!("empty rule files: {}", missing.join(", "))),
        }
    }
}

pub fn check_constitution(root: &Path) -> ReviewCheck {
    match constitution::detect(root) {
        Some(path) => ReviewCheck {
            id: "constitution".into(),
            status: CheckStatus::Passed,
            message: Some(path.display().to_string()),
        },
        None => ReviewCheck {
            id: "constitution".into(),
            status: CheckStatus::Skipped,
            message: Some("no constitution in this project".into()),
        },
    }
}
