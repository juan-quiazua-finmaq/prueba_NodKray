//! Persistent memory domain, types and repository contract (spec §46-§59, §102).
//!
//! SQLite + FTS5 is the V1 storage. The [`MemoryRepository`] trait keeps the
//! storage engine behind an interface so a future Turso sync layer can be added
//! without touching callers.

pub mod heal;
pub mod migrations;
pub mod search;
pub mod sqlite;

use serde::Serialize;

use crate::error::NodkrayResult;

/// Known memory types (spec §56). The repository accepts any string.
pub const MEMORY_TYPES: &[&str] = &[
    "decision",
    "convention",
    "bugfix",
    "discovery",
    "observation",
];

/// Generate a prefixed, temporally sortable ULID identifier (spec §112).
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", ulid::Ulid::new())
}

/// Current UTC timestamp in RFC 3339 (nanosecond precision when available).
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// A registered project (spec §49).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub git_remote: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A stored memory entry (spec §54).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Memory {
    pub id: String,
    pub project_id: String,
    #[serde(rename = "type")]
    pub memory_type: String,
    pub title: String,
    pub content: String,
    pub source: Option<String>,
    pub importance: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Search result without the full content (spec §55).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryPreview {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub memory_type: String,
    pub score: f64,
    pub preview: String,
}

/// One entry of the project timeline (memories + decisions + observations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimelineEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub created_at: String,
}

/// A stored decision (spec §53).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub question: String,
    pub decision: String,
    pub rationale: Option<String>,
    pub created_at: String,
}

/// A stored observation (spec §52).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observation {
    pub id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    #[serde(rename = "type")]
    pub observation_type: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

/// Input for [`MemoryRepository::save_memory`].
#[derive(Debug, Clone, Default)]
pub struct NewMemory {
    pub project_id: String,
    pub memory_type: String,
    pub title: String,
    pub content: String,
    pub source: Option<String>,
    pub importance: i64,
}

/// Input for [`MemoryRepository::save_decision`].
#[derive(Debug, Clone, Default)]
pub struct NewDecision {
    pub project_id: String,
    pub task_id: Option<String>,
    pub question: String,
    pub decision: String,
    pub rationale: Option<String>,
}

/// Input for [`MemoryRepository::save_observation`].
#[derive(Debug, Clone, Default)]
pub struct NewObservation {
    pub project_id: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub observation_type: String,
    pub title: String,
    pub content: String,
}

/// A persisted session (spec §50, §164).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub frontier_agent: Option<String>,
    pub workflow: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub config_snapshot: Option<String>,
}

/// Input for creating a session.
#[derive(Debug, Clone, Default)]
pub struct NewSession {
    pub project_id: String,
    pub frontier_agent: Option<String>,
    pub workflow: Option<String>,
    pub status: String,
    pub config_snapshot: Option<String>,
}

/// A worker instance (spec §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Worker {
    pub id: String,
    pub task_id: String,
    pub role: String,
    pub agent: String,
    pub worktree_path: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a worker.
#[derive(Debug, Clone, Default)]
pub struct NewWorker {
    pub task_id: String,
    pub role: String,
    pub agent: String,
    pub worktree_path: Option<String>,
    pub status: String,
}

/// A persisted review (spec §35).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Review {
    pub id: String,
    pub task_id: String,
    pub depth: String,
    pub status: String,
    pub score: Option<f64>,
    pub verdict_json: Option<String>,
    pub created_at: String,
}

/// Input for creating a review.
#[derive(Debug, Clone, Default)]
pub struct NewReview {
    pub task_id: String,
    pub depth: String,
    pub status: String,
    pub score: Option<f64>,
    pub verdict_json: Option<String>,
}

/// A single persisted review check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewCheck {
    pub id: String,
    pub review_id: String,
    pub check_id: String,
    pub status: String,
    pub detail_json: Option<String>,
}

/// Input for creating a review check.
#[derive(Debug, Clone, Default)]
pub struct NewReviewCheck {
    pub review_id: String,
    pub check_id: String,
    pub status: String,
    pub detail_json: Option<String>,
}

/// Best-effort liveness probe for a worker process (used by cancel/status).
pub fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// Storage contract for memory (spec §102).
pub trait MemoryRepository {
    /// Look up a project by canonical root path.
    fn project(&self, root_path: &str) -> NodkrayResult<Option<Project>>;

    /// Look up a project by id.
    fn project_by_id(&self, project_id: &str) -> NodkrayResult<Option<Project>>;

    /// Return the project for `root_path`, creating it on first use.
    fn get_or_create_project(&self, root_path: &str) -> NodkrayResult<Project>;

    /// Persist the git remote of a project once it is known.
    fn set_project_git_remote(&self, project_id: &str, remote: &str) -> NodkrayResult<Project>;

    /// Store a memory and return the created row.
    fn save_memory(&self, new: &NewMemory) -> NodkrayResult<Memory>;

    /// Fetch a full memory by id.
    fn get_memory(&self, id: &str) -> NodkrayResult<Option<Memory>>;

    /// Full-text search. `project_id = None` searches globally.
    fn search_memory(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> NodkrayResult<Vec<MemoryPreview>>;

    /// Delete a memory (keeps the FTS index in sync via triggers).
    fn delete_memory(&self, id: &str) -> NodkrayResult<bool>;

    /// Chronological (descending) project timeline. `None` is global.
    fn timeline(&self, project_id: Option<&str>, limit: u32) -> NodkrayResult<Vec<TimelineEntry>>;

    /// Store a decision.
    fn save_decision(&self, new: &NewDecision) -> NodkrayResult<Decision>;

    /// Store an observation.
    fn save_observation(&self, new: &NewObservation) -> NodkrayResult<Observation>;
}
