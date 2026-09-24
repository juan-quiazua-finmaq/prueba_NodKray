//! Local decision engine (spec §18-§21, §75).
//!
//! Effort is a deterministic 0..100 score derived from the task title and
//! description; no LLM is involved. Component weights (documented heuristics):
//!
//! | component             | signal                                   | weight |
//! |-----------------------|------------------------------------------|--------|
//! | change_volume         | word count (1 per 4 words)               | 0..20  |
//! | architectural_impact  | architecture/rewrite/schema/database/api | +25    |
//! | scope                 | refactor/multiple/across/migrate/rename  | +18    |
//! | uncertainty           | investigate/unknown/research/explore     | +15    |
//! | risk                  | security/auth/payment/production/drop    | +15    |
//! | dependencies          | dependency/upgrade/version/integration   | +10    |
//! | verification_cost     | test/e2e/benchmark/coverage              | +8     |
//! | user_specificity      | improve/better/cleanup/polish (vague)    | +8     |
//!
//! The score is clamped to `[0, 100]`.

use serde::Serialize;

use crate::config::schema::ThresholdsConfig;
use crate::error::NodkrayResult;

/// Review depth implemented in phase 2.
pub const REVIEW_FAST: &str = "fast";

const ARCHITECTURAL_KEYWORDS: &[&str] = &[
    "architecture",
    "architectural",
    "rewrite",
    "redesign",
    "schema",
    "database",
    "breaking",
    "api",
];

const SCOPE_KEYWORDS: &[&str] = &[
    "refactor",
    "multiple",
    "across",
    "migrate",
    "migration",
    "rename",
    "modules",
    "services",
];

const UNCERTAINTY_KEYWORDS: &[&str] = &[
    "investigate",
    "unknown",
    "research",
    "explore",
    "maybe",
    "unclear",
];

const RISK_KEYWORDS: &[&str] = &[
    "security",
    "auth",
    "payment",
    "production",
    "delete",
    "drop",
    "credentials",
];

const DEPENDENCY_KEYWORDS: &[&str] = &[
    "dependency",
    "dependencies",
    "upgrade",
    "version",
    "integration",
];

const VERIFICATION_KEYWORDS: &[&str] = &["test", "tests", "e2e", "benchmark", "coverage"];

const VAGUE_KEYWORDS: &[&str] = &["improve", "better", "cleanup", "polish", "optimize"];

/// Input for the classifier.
#[derive(Debug, Clone, Default)]
pub struct DecisionInput {
    pub title: String,
    pub description: String,
}

/// Decision Schema output (spec §75).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionOutput {
    pub workflow: String,
    pub effort: u32,
    pub review_depth: String,
    pub reasons: Vec<String>,
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

/// Deterministic effort score plus human-readable reasons.
pub fn score_effort(input: &DecisionInput) -> (u32, Vec<String>) {
    let text = format!("{} {}", input.title, input.description).to_lowercase();
    let words = text.split_whitespace().count();
    let mut score: i32 = 0;
    let mut reasons: Vec<String> = Vec::new();

    let change_volume = ((words as i32) / 4).min(20);
    score += change_volume;
    if change_volume >= 10 {
        reasons.push("large change volume".to_string());
    }

    if contains_any(&text, ARCHITECTURAL_KEYWORDS) {
        score += 25;
        reasons.push("possible architectural impact".to_string());
    }
    if contains_any(&text, SCOPE_KEYWORDS) {
        score += 18;
        reasons.push("multiple areas affected".to_string());
    }
    if contains_any(&text, UNCERTAINTY_KEYWORDS) {
        score += 15;
        reasons.push("moderate uncertainty".to_string());
    }
    if contains_any(&text, RISK_KEYWORDS) {
        score += 15;
        reasons.push("risk-sensitive area".to_string());
    }
    if contains_any(&text, DEPENDENCY_KEYWORDS) {
        score += 10;
        reasons.push("external dependencies".to_string());
    }
    if contains_any(&text, VERIFICATION_KEYWORDS) {
        score += 8;
        reasons.push("higher verification cost".to_string());
    }
    if contains_any(&text, VAGUE_KEYWORDS) {
        score += 8;
        reasons.push("low requirement specificity".to_string());
    }

    if reasons.is_empty() {
        reasons.push("localized, low-uncertainty change".to_string());
    }

    (score.clamp(0, 100) as u32, reasons)
}

/// Message used when `--workflow st` forces ST above the threshold.
pub fn force_st_warning(effort: u32, st_max: u32) -> String {
    format!("effort {effort} > st_max {st_max}, forcing ST")
}

/// Classify a task into ST / ODD / SDD from effort thresholds.
pub fn decide(
    input: &DecisionInput,
    thresholds: &ThresholdsConfig,
    force_st: bool,
) -> NodkrayResult<DecisionOutput> {
    let (effort, reasons) = score_effort(input);

    if force_st || effort <= thresholds.st_max {
        return Ok(DecisionOutput {
            workflow: "ST".to_string(),
            effort,
            review_depth: REVIEW_FAST.to_string(),
            reasons,
        });
    }

    if effort <= thresholds.odd_max {
        return Ok(DecisionOutput {
            workflow: "ODD".to_string(),
            effort,
            review_depth: "balanced".to_string(),
            reasons,
        });
    }

    Ok(DecisionOutput {
        workflow: "SDD".to_string(),
        effort,
        review_depth: "deep".to_string(),
        reasons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> ThresholdsConfig {
        ThresholdsConfig {
            st_max: 20,
            odd_max: 60,
        }
    }

    #[test]
    fn small_task_scores_low_and_is_st() {
        let input = DecisionInput {
            title: "fix typo".to_string(),
            description: "Fix a typo in the README".to_string(),
        };
        let (effort, _) = score_effort(&input);
        assert!(effort <= 20, "effort={effort}");
        let decision = decide(&input, &thresholds(), false).expect("st");
        assert_eq!(decision.workflow, "ST");
        assert_eq!(decision.review_depth, "fast");
    }

    #[test]
    fn large_architectural_task_selects_odd_or_sdd() {
        let input = DecisionInput {
            title: "Refactor architecture across multiple modules".to_string(),
            description: "Redesign the database schema and migrate all services with breaking API changes for production".to_string(),
        };
        let (effort, reasons) = score_effort(&input);
        assert!(effort > 20, "effort={effort}, reasons={reasons:?}");
        let decision = decide(&input, &thresholds(), false).expect("classified");
        assert!(decision.workflow == "ODD" || decision.workflow == "SDD");
    }

    #[test]
    fn force_st_overrides_the_threshold() {
        let input = DecisionInput {
            title: "Refactor architecture across multiple modules".to_string(),
            description: "Redesign the database schema and migrate all services".to_string(),
        };
        let decision = decide(&input, &thresholds(), true).expect("forced");
        assert_eq!(decision.workflow, "ST");
    }

    #[test]
    fn warning_matches_required_text() {
        assert_eq!(
            force_st_warning(47, 20),
            "effort 47 > st_max 20, forcing ST"
        );
    }
}
