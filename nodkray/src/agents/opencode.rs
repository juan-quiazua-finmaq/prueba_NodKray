//! OpenCode adapter stub (spec §9.2). Headless execution lands in phase 4.

use super::traits::StubAdapter;

/// Registry entry for the OpenCode provider.
pub fn adapter() -> StubAdapter {
    StubAdapter::new("opencode", "opencode")
}
