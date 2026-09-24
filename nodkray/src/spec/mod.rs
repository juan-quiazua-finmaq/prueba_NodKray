//! Spec-Kit integration (spec §26-§28).

pub mod constitution;
pub mod speckit;

pub use constitution::{detect as detect_constitution, ensure as ensure_constitution};
pub use speckit::{SpecKitAdapter, SpecKitStage};
