//! MCP discovery (spec §40-§43, §96).

pub mod discovery;

pub use discovery::{discover, select_for_role, McpServer};
