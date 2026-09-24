//! Execution backends (spec §13-§16, §80-§81).

pub mod console;
pub mod dag;
pub mod herdr;
pub mod registry;
pub mod traits;
pub mod worktree;

pub use console::ConsoleBackend;
pub use dag::{schedule, DagNode};
pub use herdr::HerdrBackend;
pub use registry::select as select_backend;
pub use traits::{
    ExecutionBackend, WorkerEvent, WorkerOutcome, WorkerSpec, WorkerStatus, Workspace,
    WorkspaceRequest,
};
pub use worktree::{assert_isolated, worktree_path};
