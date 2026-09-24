//! Codex adapter stub (spec §9.4). Headless execution lands in phase 4.

use super::traits::StubAdapter;

/// Registry entry for the Codex provider.
pub fn adapter() -> StubAdapter {
    StubAdapter::new("codex", "codex")
}
