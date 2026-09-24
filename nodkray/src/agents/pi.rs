//! Pi adapter stub (spec §9.5). Headless execution lands in phase 4.

use super::traits::StubAdapter;

/// Registry entry for the Pi provider.
pub fn adapter() -> StubAdapter {
    StubAdapter::new("pi", "pi")
}
