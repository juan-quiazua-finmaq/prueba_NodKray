//! RDD review engine (spec §29-§36, §102).

use std::path::Path;

use crate::config::Config;
use crate::core::workflow::ReviewDepth;

use super::agent::check_agent_review;
use super::git::{check_diff, detect_conflict};
use super::lint::check_lint;
use super::rules::{check_constitution, check_rules};
use super::sentrux::check_sentrux;
use super::tests::check_tests;
use super::types::{evaluate_policy, ReviewPolicy, Verdict};

pub struct ReviewRequest<'a> {
    pub root: &'a Path,
    pub depth: ReviewDepth,
    pub config: &'a Config,
    pub reviewer_available: bool,
}

pub fn review(request: ReviewRequest<'_>) -> Verdict {
    let mut checks = Vec::new();
    checks.push(check_diff(request.root));
    checks.push(detect_conflict(request.root));
    checks.push(check_tests(request.root));
    checks.push(check_rules(request.root));
    checks.push(check_constitution(request.root));

    if matches!(request.depth, ReviewDepth::Balanced | ReviewDepth::Deep) {
        checks.push(check_lint(request.root));
        checks.push(check_sentrux(
            request.root,
            request.config.integrations.sentrux.enabled,
            request.config.integrations.sentrux.required,
        ));
    }
    if request.depth == ReviewDepth::Deep {
        checks.push(check_agent_review(
            request.root,
            request.depth,
            request.reviewer_available,
        ));
    }

    let policy = ReviewPolicy::from_config(
        &request.config.review.policy,
        request.config.integrations.sentrux.required,
    );
    evaluate_policy(checks, &policy)
}

/// After a failed verdict the task returns to REVIEWING (spec §17).
pub fn next_status_after(verdict: &Verdict) -> &'static str {
    match verdict.status {
        super::types::VerdictStatus::Passed => "PASSED",
        super::types::VerdictStatus::Failed | super::types::VerdictStatus::Blocked => "REMEDIATING",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn fast_review_does_not_require_sentrux() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("README.md"), "x\n").expect("write");
        let config = Config::default();
        let verdict = review(ReviewRequest {
            root: tmp.path(),
            depth: ReviewDepth::Fast,
            config: &config,
            reviewer_available: false,
        });
        assert!(verdict.checks.iter().all(|c| c.id != "sentrux"));
        assert!(verdict.checks.iter().all(|c| c.id != "agent-review"));
    }
}
