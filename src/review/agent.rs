//! Review-agent check (spec §82-§83). Builds the review prompt; does not invent a pass.

use std::path::Path;

use super::types::{CheckStatus, ReviewCheck};
use crate::core::workflow::ReviewDepth;

/// Input the reviewer is allowed to see — never the full chat history.
#[derive(Debug, Clone)]
pub struct ReviewerInput {
    pub task_specification: String,
    pub project_rules: Vec<String>,
    pub changed_files: Vec<String>,
    pub git_diff: String,
    pub test_results: String,
    pub sentrux_results: String,
}

pub fn build_prompt(input: &ReviewerInput) -> String {
    format!(
        "Review the following work against the task and project rules.\n\n# Task\n{}\n\n# Rules\n{}\n\n# Changed files\n{}\n\n# Git diff\n{}\n\n# Tests\n{}\n\n# Sentrux\n{}\n",
        input.task_specification,
        input.project_rules.join("\n"),
        input.changed_files.join("\n"),
        input.git_diff,
        input.test_results,
        input.sentrux_results
    )
}

pub fn check_agent_review(
    root: &Path,
    depth: ReviewDepth,
    reviewer_available: bool,
    executed: bool,
) -> ReviewCheck {
    let _ = root;
    if depth != ReviewDepth::Deep {
        return ReviewCheck {
            id: "agent-review".into(),
            status: CheckStatus::Skipped,
            message: Some("agent review runs only at deep depth".into()),
        };
    }
    if !reviewer_available {
        return ReviewCheck {
            id: "agent-review".into(),
            status: CheckStatus::Blocked,
            message: Some("reviewer agent is required for deep RDD and is not available".into()),
        };
    }
    if !executed {
        return ReviewCheck {
            id: "agent-review".into(),
            status: CheckStatus::Blocked,
            message: Some("reviewer agent was not executed; refusing a silent pass".into()),
        };
    }
    ReviewCheck {
        id: "agent-review".into(),
        status: CheckStatus::Passed,
        message: Some("reviewer completed".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_review_without_execution_is_blocked() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let check = check_agent_review(tmp.path(), ReviewDepth::Deep, true, false);
        assert_eq!(check.status, CheckStatus::Blocked);
    }

    #[test]
    fn prompt_excludes_chat_history_heading() {
        let prompt = build_prompt(&ReviewerInput {
            task_specification: "Add auth".into(),
            project_rules: vec!["no secrets".into()],
            changed_files: vec!["src/auth.rs".into()],
            git_diff: "diff".into(),
            test_results: "ok".into(),
            sentrux_results: "ok".into(),
        });
        assert!(prompt.contains("# Task"));
        assert!(!prompt.to_lowercase().contains("conversation history"));
    }
}
