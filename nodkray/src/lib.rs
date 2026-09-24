//! NodKray — local-first agent orchestration CLI.
//!
//! The crate is intentionally a single binary with a library target so that
//! integration tests can exercise domain logic without spawning a process.
//!
//! Module layout follows spec §6. Most modules are scaffolds for later phases;
//! allow(dead_code) is scoped to the crate while those stubs are empty and is
//! expected to be removed as implementation lands.

#![allow(dead_code)]

pub mod agents;
pub mod cli;
pub mod config;
pub mod core;
pub mod error;
pub mod execution;
pub mod installer;
pub mod logging;
pub mod mcp;
pub mod memory;
pub mod remote;
pub mod review;
pub mod spec;

pub use error::{ErrorCategory, ErrorDetail, NodkrayError, NodkrayResult};
