//! Review depth selection from effort and workflow (spec §31-§32, §85).

use crate::config::schema::ReviewThresholdsConfig;

/// RDD depth levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDepth {
    Fast,
    Balanced,
    Deep,
}

impl ReviewDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewDepth::Fast => "fast",
            ReviewDepth::Balanced => "balanced",
            ReviewDepth::Deep => "deep",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fast" => Some(Self::Fast),
            "balanced" => Some(Self::Balanced),
            "deep" => Some(Self::Deep),
            _ => None,
        }
    }
}

/// Workflow kinds selected by the decision engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkflowKind {
    St,
    Odd,
    Sdd,
}

impl WorkflowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkflowKind::St => "ST",
            WorkflowKind::Odd => "ODD",
            WorkflowKind::Sdd => "SDD",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "ST" => Some(Self::St),
            "ODD" => Some(Self::Odd),
            "SDD" => Some(Self::Sdd),
            _ => None,
        }
    }

    /// Default depth when the user does not force one (spec §85).
    pub fn default_depth(self) -> ReviewDepth {
        match self {
            WorkflowKind::St => ReviewDepth::Fast,
            WorkflowKind::Odd => ReviewDepth::Balanced,
            WorkflowKind::Sdd => ReviewDepth::Deep,
        }
    }
}

/// Select RDD depth: explicit override wins, else effort thresholds, else workflow default.
pub fn select_review_depth(
    workflow: WorkflowKind,
    effort: u32,
    thresholds: &ReviewThresholdsConfig,
    override_depth: Option<ReviewDepth>,
) -> ReviewDepth {
    if let Some(forced) = override_depth {
        return forced;
    }
    if effort <= thresholds.fast_max {
        return ReviewDepth::Fast;
    }
    if effort <= thresholds.balanced_max {
        return ReviewDepth::Balanced;
    }
    // SDD defaults to deep above the balanced threshold; others stay deep too.
    let _ = workflow;
    ReviewDepth::Deep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> ReviewThresholdsConfig {
        ReviewThresholdsConfig::default()
    }

    #[test]
    fn override_wins_over_effort() {
        let depth = select_review_depth(WorkflowKind::St, 90, &thresholds(), Some(ReviewDepth::Fast));
        assert_eq!(depth, ReviewDepth::Fast);
    }

    #[test]
    fn effort_maps_to_thresholds() {
        assert_eq!(
            select_review_depth(WorkflowKind::Odd, 10, &thresholds(), None),
            ReviewDepth::Fast
        );
        assert_eq!(
            select_review_depth(WorkflowKind::Odd, 40, &thresholds(), None),
            ReviewDepth::Balanced
        );
        assert_eq!(
            select_review_depth(WorkflowKind::Sdd, 80, &thresholds(), None),
            ReviewDepth::Deep
        );
    }

    #[test]
    fn workflow_defaults_match_spec_85() {
        assert_eq!(WorkflowKind::St.default_depth(), ReviewDepth::Fast);
        assert_eq!(WorkflowKind::Odd.default_depth(), ReviewDepth::Balanced);
        assert_eq!(WorkflowKind::Sdd.default_depth(), ReviewDepth::Deep);
    }
}
