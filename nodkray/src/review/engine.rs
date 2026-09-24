//! RDD engine (spec §29-§36). Phase 2 implements FAST only.
//!
//! A required check that is missing or failing never yields a pass (spec §36,
//! §114): tests unavailable → `blocked`; tests failed → `failed`.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{git, rules, tests as test_runner};
use crate::config::schema::ReviewPolicyConfig;
use crate::error::{NodkrayError, NodkrayResult};
use crate::memory::sqlite::SqliteMemoryRepository;
use crate::memory::{NewReview, NewReviewCheck};

/// One check in the verdict (spec §35).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// A detected violation (spec §35).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub id: String,
    pub severity: String,
    pub source: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Normalised review result (spec §35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewVerdict {
    /// `passed`, `failed` or `blocked`.
    pub status: String,
    #[serde(default)]
    pub score: Option<f64>,
    pub checks: Vec<CheckResult>,
    #[serde(default)]
    pub violations: Vec<Violation>,
    pub remediation_required: bool,
}

/// Everything needed for a FAST review.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub task_id: String,
    pub depth: String,
    pub root: PathBuf,
    pub worktree: Option<PathBuf>,
    pub base_branch: String,
    pub policy: ReviewPolicyConfig,
    pub test_timeout: Duration,
}

/// Run a FAST review and persist the verdict plus its checks.
pub fn run_fast(
    repo: &SqliteMemoryRepository,
    request: &ReviewRequest,
) -> NodkrayResult<ReviewVerdict> {
    let dir = request
        .worktree
        .clone()
        .unwrap_or_else(|| request.root.clone());

    let mut checks = Vec::new();

    // 1. git diff.
    let changed = git::changed_files(&dir, &request.base_branch).unwrap_or_default();
    checks.push(CheckResult {
        id: "git-diff".to_string(),
        status: "passed".to_string(),
        detail: format!("{} changed file(s)", changed.len()),
    });

    // 2. tests.
    let test_result = test_runner::run_tests(&dir, request.test_timeout)?;
    checks.push(CheckResult {
        id: "tests".to_string(),
        status: test_result.status,
        detail: test_result.detail,
    });

    // 3. rules (registration only).
    let rules_check = rules::check_rules(&dir);
    checks.push(CheckResult {
        id: "rules".to_string(),
        status: rules_check.status,
        detail: rules_check.detail,
    });

    // Policy (spec §36).
    let mut status = "passed".to_string();
    let mut violations = Vec::new();
    for check in &checks {
        if check.id != "tests" {
            continue;
        }
        let blocks = request.policy.test_failure == "block";
        match check.status.as_str() {
            "failed" if blocks => {
                status = "failed".to_string();
                violations.push(Violation {
                    id: "tests.failed".to_string(),
                    severity: "critical".to_string(),
                    source: "tests".to_string(),
                    message: check.detail.clone(),
                    path: None,
                });
            }
            "unavailable" if blocks => {
                status = "blocked".to_string();
                violations.push(Violation {
                    id: "tests.unavailable".to_string(),
                    severity: "critical".to_string(),
                    source: "tests".to_string(),
                    message: check.detail.clone(),
                    path: None,
                });
            }
            _ => {}
        }
    }

    let remediation_required = status == "failed";
    let verdict = ReviewVerdict {
        status,
        score: None,
        checks,
        violations,
        remediation_required,
    };

    persist(repo, request, &verdict)?;
    Ok(verdict)
}

fn persist(
    repo: &SqliteMemoryRepository,
    request: &ReviewRequest,
    verdict: &ReviewVerdict,
) -> NodkrayResult<()> {
    let verdict_json = serde_json::to_string(verdict)
        .map_err(|err| NodkrayError::internal("REVIEW_SERIALIZE_FAILED", err.to_string()))?;
    let review = repo.create_review(&NewReview {
        task_id: request.task_id.clone(),
        depth: request.depth.clone(),
        status: verdict.status.clone(),
        score: verdict.score,
        verdict_json: Some(verdict_json),
    })?;
    for check in &verdict.checks {
        repo.add_review_check(&NewReviewCheck {
            review_id: review.id.clone(),
            check_id: check.id.clone(),
            status: check.status.clone(),
            detail_json: Some(serde_json::json!({ "detail": check.detail }).to_string()),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_serialises_to_spec_shape() {
        let verdict = ReviewVerdict {
            status: "passed".to_string(),
            score: None,
            checks: vec![CheckResult {
                id: "git-diff".to_string(),
                status: "passed".to_string(),
                detail: "1 changed file(s)".to_string(),
            }],
            violations: Vec::new(),
            remediation_required: false,
        };
        let value = serde_json::to_value(&verdict).expect("json");
        assert_eq!(value["status"], "passed");
        assert!(value["score"].is_null());
        assert_eq!(value["checks"][0]["id"], "git-diff");
        assert_eq!(value["remediation_required"], false);
    }
}
