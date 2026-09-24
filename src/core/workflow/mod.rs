//! Workflow engine (spec §21-§23, §102).

pub mod depth;
pub mod odd;
pub mod sdd;
pub mod st;

pub use depth::{select_review_depth, ReviewDepth, WorkflowKind};
pub use odd::{write_task_md, OddTask};
pub use sdd::{prepare as prepare_sdd, SddPlan};
pub use st::{
    make_title, require_description, run_st, MergeSummary, ReviewSummary, StOutcome, StRun,
    WorkerSummary, WorkflowRequest,
};
