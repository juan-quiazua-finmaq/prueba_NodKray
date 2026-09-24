//! Workflow engine surface (spec §21-§23, §102).
//!
//! ST remains owned by fase 2. This module adds ODD, SDD coordination and
//! review-depth selection without calling Herdr or `specify` directly.

pub mod depth;
pub mod odd;
pub mod sdd;

pub use depth::{select_review_depth, ReviewDepth, WorkflowKind};
pub use odd::{write_task_md, OddTask};
pub use sdd::{prepare as prepare_sdd, SddPlan};
