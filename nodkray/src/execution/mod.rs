//! Execution backends (spec §13-§15). Phase 2 implements the Console backend.

pub mod console;
pub mod herdr;
pub mod traits;

pub use console::ConsoleBackend;
pub use traits::{
    ExecutionBackend, WorkerEvent, WorkerOutcome, WorkerSpec, WorkerStatus, Workspace,
    WorkspaceRequest,
};
