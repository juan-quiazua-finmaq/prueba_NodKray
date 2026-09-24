//! Workflow engine (spec §21, §102). Phase 2 implements ST end-to-end.

pub mod st;

pub use st::{
    make_title, require_description, run_st, MergeSummary, ReviewSummary, StOutcome, StRun,
    WorkerSummary, WorkflowRequest,
};
