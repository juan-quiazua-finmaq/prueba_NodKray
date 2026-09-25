//! Normalised RDD types (spec §35-§36).

use serde::{Deserialize, Serialize};

use crate::config::schema::ReviewPolicyConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Passed,
    Failed,
    Blocked,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewCheck {
    pub id: String,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewViolation {
    pub id: String,
    pub severity: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerdictStatus {
    Passed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub status: VerdictStatus,
    pub score: Option<f64>,
    pub checks: Vec<ReviewCheck>,
    pub violations: Vec<ReviewViolation>,
    pub remediation_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Block,
    Allow,
}

impl PolicyAction {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "allow" | "warn" | "configurable" => Self::Allow,
            _ => Self::Block,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReviewPolicy {
    pub test_failure: PolicyAction,
    pub constitution_violation: PolicyAction,
    pub sentrux_failure: PolicyAction,
    pub lint_failure: PolicyAction,
    pub reviewer_failure: PolicyAction,
    pub sentrux_required: bool,
}

impl ReviewPolicy {
    pub fn from_config(config: &ReviewPolicyConfig, sentrux_required: bool) -> Self {
        Self {
            test_failure: PolicyAction::parse(&config.test_failure),
            constitution_violation: PolicyAction::parse(&config.constitution_violation),
            sentrux_failure: PolicyAction::parse(&config.sentrux_failure),
            lint_failure: PolicyAction::parse(&config.lint_failure),
            reviewer_failure: PolicyAction::parse(&config.reviewer_failure),
            sentrux_required,
        }
    }

    fn action_for(&self, check_id: &str) -> PolicyAction {
        match check_id {
            "tests" => self.test_failure,
            "rules" | "constitution" => self.constitution_violation,
            "sentrux" => self.sentrux_failure,
            "lint" => self.lint_failure,
            "agent-review" => self.reviewer_failure,
            _ => PolicyAction::Block,
        }
    }

    fn mandatory(&self, check_id: &str) -> bool {
        match check_id {
            "sentrux" => self.sentrux_required,
            "git-diff" | "tests" | "rules" | "isolation" => true,
            "agent-review" => self.reviewer_failure == PolicyAction::Block,
            _ => false,
        }
    }
}

/// Evaluate checks against policy. A missing mandatory check is BLOCKED, never passed.
pub fn evaluate_policy(checks: Vec<ReviewCheck>, policy: &ReviewPolicy) -> Verdict {
    let mut violations = Vec::new();
    let mut blocked = false;
    let mut failed = false;

    for check in &checks {
        match check.status {
            CheckStatus::Passed => {}
            CheckStatus::Skipped => {
                if policy.mandatory(&check.id) {
                    blocked = true;
                    violations.push(ReviewViolation {
                        id: format!("{}.unavailable", check.id),
                        severity: "critical".into(),
                        source: check.id.clone(),
                        message: check
                            .message
                            .clone()
                            .unwrap_or_else(|| format!("{} is mandatory but was not available", check.id)),
                        path: None,
                    });
                }
            }
            CheckStatus::Blocked => {
                blocked = true;
                violations.push(violation_from(check, "critical"));
            }
            CheckStatus::Failed => {
                if policy.action_for(&check.id) == PolicyAction::Block {
                    failed = true;
                    violations.push(violation_from(check, "critical"));
                }
            }
        }
    }

    let status = if blocked {
        VerdictStatus::Blocked
    } else if failed {
        VerdictStatus::Failed
    } else {
        VerdictStatus::Passed
    };

    Verdict {
        status,
        score: None,
        checks,
        violations,
        remediation_required: status != VerdictStatus::Passed,
    }
}

fn violation_from(check: &ReviewCheck, severity: &str) -> ReviewViolation {
    ReviewViolation {
        id: check.id.clone(),
        severity: severity.to_string(),
        source: check.id.clone(),
        message: check
            .message
            .clone()
            .unwrap_or_else(|| format!("{} failed", check.id)),
        path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, status: CheckStatus) -> ReviewCheck {
        ReviewCheck {
            id: id.into(),
            status,
            message: None,
        }
    }

    fn policy() -> ReviewPolicy {
        ReviewPolicy {
            test_failure: PolicyAction::Block,
            constitution_violation: PolicyAction::Block,
            sentrux_failure: PolicyAction::Block,
            lint_failure: PolicyAction::Allow,
            reviewer_failure: PolicyAction::Block,
            sentrux_required: false,
        }
    }

    #[test]
    fn unavailable_mandatory_check_blocks() {
        let verdict = evaluate_policy(vec![check("tests", CheckStatus::Skipped)], &policy());
        assert_eq!(verdict.status, VerdictStatus::Blocked);
        assert!(verdict.remediation_required);
    }

    #[test]
    fn lint_failure_can_be_allowed() {
        let verdict = evaluate_policy(
            vec![
                check("git-diff", CheckStatus::Passed),
                check("tests", CheckStatus::Passed),
                check("lint", CheckStatus::Failed),
            ],
            &policy(),
        );
        assert_eq!(verdict.status, VerdictStatus::Passed);
    }

    #[test]
    fn test_failure_blocks() {
        let verdict = evaluate_policy(vec![check("tests", CheckStatus::Failed)], &policy());
        assert_eq!(verdict.status, VerdictStatus::Failed);
    }

    #[test]
    fn optional_sentrux_skip_does_not_block() {
        let verdict = evaluate_policy(vec![check("sentrux", CheckStatus::Skipped)], &policy());
        assert_eq!(verdict.status, VerdictStatus::Passed);
    }

    #[test]
    fn required_sentrux_skip_blocks() {
        let mut p = policy();
        p.sentrux_required = true;
        let verdict = evaluate_policy(vec![check("sentrux", CheckStatus::Skipped)], &p);
        assert_eq!(verdict.status, VerdictStatus::Blocked);
    }
}
