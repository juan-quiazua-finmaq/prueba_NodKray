//! Cursor adapter stub (spec §9.1). Headless execution lands in phase 4.

use super::traits::StubAdapter;

/// Registry entry for the Cursor provider.
pub fn adapter() -> StubAdapter {
    StubAdapter::new("cursor", "cursor-agent")
}
