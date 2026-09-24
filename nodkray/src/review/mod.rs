//! Review Driven Development (spec §29-§39).

pub mod engine;
pub mod git;
pub mod rules;
pub mod sentrux;
pub mod tests;

pub use engine::{run_fast, CheckResult, ReviewRequest, ReviewVerdict, Violation};
pub use git::{merge_branch, MergeOutcome, MergeStatus};
