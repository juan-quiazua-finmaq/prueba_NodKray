//! RDD engine (spec §29-§36).
//!
//! A required check that is missing or failing never yields a pass (spec §36,
//! §114). Policy evaluation goes through [`evaluate_policy`].

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::types::{
    evaluate_policy, CheckStatus, ReviewCheck, ReviewPolicy, Verdict, VerdictStatus,
};
use super::{agent, git, lint, rules, sentrux, tests as test_runner};
use crate::config::schema::ReviewPolicyConfig;
use crate::config::Config;
use crate::core::workflow::ReviewDepth;
use crate::error::NodkrayResult;
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

/// Inputs for a review at any RDD depth.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub task_id: String,
    pub depth: String,
    pub root: PathBuf,
    pub worktree: Option<PathBuf>,
    pub base_branch: String,
    pub policy: ReviewPolicyConfig,
    pub test_timeout: Duration,
    pub sentrux_enabled: bool,
    pub sentrux_required: bool,
    pub reviewer_available: bool,
}

impl ReviewRequest {
    /// Fill integration flags from the live project config.
    pub fn with_config(mut self, config: &Config) -> Self {
        self.sentrux_enabled = config.integrations.sentrux.enabled;
        self.sentrux_required = config.integrations.sentrux.required;
        self.policy = config.review.policy.clone();
        self
    }
}

/// Backward-compatible alias: FAST review with the same policy engine.
pub fn run_fast(
    repo: &SqliteMemoryRepository,
    request: &ReviewRequest,
) -> NodkrayResult<ReviewVerdict> {
    run_review(repo, request)
}

/// Run RDD at the requested depth and persist the verdict plus its checks.
pub fn run_review(
    repo: &SqliteMemoryRepository,
    request: &ReviewRequest,
) -> NodkrayResult<ReviewVerdict> {
    let dir = request
        .worktree
        .clone()
        .unwrap_or_else(|| request.root.clone());
    let depth = ReviewDepth::parse(&request.depth).unwrap_or(ReviewDepth::Fast);
    let policy = ReviewPolicy::from_config(&request.policy, request.sentrux_required);

    let mut checks = Vec::new();
    checks.push(git::check_diff(&dir, &request.base_branch));
    checks.push(test_check(&dir, request.test_timeout)?);
    checks.push(rules_check(&dir));

    if depth == ReviewDepth::Balanced || depth == ReviewDepth::Deep {
        checks.push(lint::check_lint(&dir));
        checks.push(sentrux::check_sentrux(
            &dir,
            request.sentrux_enabled,
            request.sentrux_required,
        ));
    }
    if depth == ReviewDepth::Deep {
        // Do not invent a pass: a reviewer that was not executed is BLOCKED.
        checks.push(agent::check_agent_review(
            &dir,
            depth,
            request.reviewer_available,
            false,
        ));
    }

    let verdict = from_internal(evaluate_policy(checks, &policy));
    persist(repo, request, &verdict)?;
    Ok(verdict)
}

fn test_check(dir: &std::path::Path, timeout: Duration) -> NodkrayResult<ReviewCheck> {
    let result = test_runner::run_tests(dir, timeout)?;
    let status = match result.status.as_str() {
        "passed" => CheckStatus::Passed,
        "failed" => CheckStatus::Failed,
        _ => CheckStatus::Skipped,
    };
    Ok(ReviewCheck {
        id: "tests".to_string(),
        status,
        message: Some(result.detail),
    })
}

fn rules_check(dir: &std::path::Path) -> ReviewCheck {
    let result = rules::check_rules(dir);
    ReviewCheck {
        id: "rules".to_string(),
        status: CheckStatus::Passed,
        message: Some(result.detail),
    }
}

fn from_internal(verdict: Verdict) -> ReviewVerdict {
    ReviewVerdict {
        status: match verdict.status {
            VerdictStatus::Passed => "passed",
            VerdictStatus::Failed => "failed",
            VerdictStatus::Blocked => "blocked",
        }
        .to_string(),
        score: verdict.score,
        checks: verdict
            .checks
            .into_iter()
            .map(|check| CheckResult {
                id: check.id,
                status: match check.status {
                    CheckStatus::Passed => "passed",
                    CheckStatus::Failed => "failed",
                    CheckStatus::Blocked => "blocked",
                    CheckStatus::Skipped => "unavailable",
                }
                .to_string(),
                detail: check.message.unwrap_or_default(),
            })
            .collect(),
        violations: verdict
            .violations
            .into_iter()
            .map(|item| Violation {
                id: item.id,
                severity: item.severity,
                source: item.source,
                message: item.message,
                path: item.path,
            })
            .collect(),
        remediation_required: verdict.remediation_required,
    }
}

fn persist(
    repo: &SqliteMemoryRepository,
    request: &ReviewRequest,
    verdict: &ReviewVerdict,
) -> NodkrayResult<()> {
    let verdict_json = serde_json::to_string(verdict).map_err(|err| {
        crate::error::NodkrayError::internal("REVIEW_SERIALIZE_FAILED", err.to_string())
    })?;
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
    use crate::memory::sqlite::SqliteMemoryRepository;
    use crate::memory::MemoryRepository;

    fn seeded_request(
        repo: &SqliteMemoryRepository,
        root: PathBuf,
    ) -> ReviewRequest {
        let project = repo
            .get_or_create_project(&root.to_string_lossy())
            .expect("project");
        let task = crate::core::task::create_task(
            repo,
            &crate::core::task::NewTask::new(project.id, "review seed", "desc"),
        )
        .expect("task");
        ReviewRequest {
            task_id: task.id,
            depth: "fast".to_string(),
            root,
            worktree: None,
            base_branch: "HEAD".to_string(),
            policy: ReviewPolicyConfig::default(),
            test_timeout: Duration::from_secs(5),
            sentrux_enabled: false,
            sentrux_required: false,
            reviewer_available: false,
        }
    }

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

    #[test]
    fn missing_git_repo_blocks_instead_of_passing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("memory.db");
        let repo = SqliteMemoryRepository::open(&db).expect("db");
        let verdict = run_review(&repo, &seeded_request(&repo, tmp.path().to_path_buf())).expect("review");
        assert_eq!(verdict.status, "blocked");
        let git = verdict
            .checks
            .iter()
            .find(|check| check.id == "git-diff")
            .expect("git-diff");
        assert_ne!(git.status, "passed");
    }

    #[test]
    fn missing_tests_block_when_policy_requires_them() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).expect("repo");
        let status = std::process::Command::new("git")
            .args(["-C", &repo_dir.to_string_lossy(), "init", "-q"])
            .status()
            .expect("git init");
        assert!(status.success());
        let db = tmp.path().join("memory.db");
        let repo = SqliteMemoryRepository::open(&db).expect("db");
        let verdict = run_review(&repo, &seeded_request(&repo, repo_dir)).expect("review");
        assert_eq!(verdict.status, "blocked");
        assert!(verdict
            .violations
            .iter()
            .any(|item| item.id.contains("tests")));
    }
}
