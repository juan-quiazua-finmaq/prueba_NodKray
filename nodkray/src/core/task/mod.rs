//! Task model and persistence surface (spec §17, §51).

use serde::{Deserialize, Serialize};

/// Minimal task projection used by `nodkray status` / `task inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub state: String,
}

/// Active (non-terminal) tasks.
///
/// TODO(Fase 1): read from SQLite. Phase 0 has no persistence, so it is
/// deliberately empty.
pub fn active_tasks() -> Vec<TaskSummary> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase0_reports_no_active_tasks() {
        assert!(active_tasks().is_empty());
    }
}
