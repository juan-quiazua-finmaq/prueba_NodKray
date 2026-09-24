//! Task model and persistence surface (spec §17, §51, §102, §112).
//!
//! State transitions are persisted together with a `task_events` row inside the
//! same transaction, so a crash can never lose the status change.

use serde::{Deserialize, Serialize};

use crate::error::{NodkrayError, NodkrayResult};

/// Lifecycle states (spec §17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Pending,
    Classifying,
    Planned,
    Running,
    Reviewing,
    Remediating,
    Passed,
    Failed,
    Merged,
    Conflict,
    Blocked,
    Cancelled,
}

/// States after which a task is no longer "active".
pub const TERMINAL_STATES: &[&str] = &[
    "PASSED",
    "FAILED",
    "MERGED",
    "CONFLICT",
    "BLOCKED",
    "CANCELLED",
];

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "PENDING",
            TaskStatus::Classifying => "CLASSIFYING",
            TaskStatus::Planned => "PLANNED",
            TaskStatus::Running => "RUNNING",
            TaskStatus::Reviewing => "REVIEWING",
            TaskStatus::Remediating => "REMEDIATING",
            TaskStatus::Passed => "PASSED",
            TaskStatus::Failed => "FAILED",
            TaskStatus::Merged => "MERGED",
            TaskStatus::Conflict => "CONFLICT",
            TaskStatus::Blocked => "BLOCKED",
            TaskStatus::Cancelled => "CANCELLED",
        }
    }

    /// Parse a persisted status. Unknown values fall back to `PENDING` with a
    /// warning rather than aborting the command.
    pub fn parse(value: &str) -> Self {
        match value {
            "CLASSIFYING" => TaskStatus::Classifying,
            "PLANNED" => TaskStatus::Planned,
            "RUNNING" => TaskStatus::Running,
            "REVIEWING" => TaskStatus::Reviewing,
            "REMEDIATING" => TaskStatus::Remediating,
            "PASSED" => TaskStatus::Passed,
            "FAILED" => TaskStatus::Failed,
            "MERGED" => TaskStatus::Merged,
            "CONFLICT" => TaskStatus::Conflict,
            "BLOCKED" => TaskStatus::Blocked,
            "CANCELLED" => TaskStatus::Cancelled,
            "PENDING" => TaskStatus::Pending,
            other => {
                tracing::warn!(status = other, "unknown task status; defaulting to PENDING");
                TaskStatus::Pending
            }
        }
    }

    pub fn is_terminal(self) -> bool {
        TERMINAL_STATES.contains(&self.as_str())
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}

/// A persisted task (spec §51).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: String,
    pub workflow: String,
    pub effort: Option<i64>,
    pub role: Option<String>,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// List projection used by `nodkray status` / `task inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub workflow: String,
    pub updated_at: String,
}

/// A persisted task event (spec §52).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskEvent {
    pub id: String,
    pub task_id: String,
    pub event: String,
    pub payload: Option<serde_json::Value>,
    pub created_at: String,
}

/// Input for [`TaskRepository::create_task`].
#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub project_id: String,
    pub session_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: String,
    pub workflow: String,
    pub effort: Option<i64>,
    pub role: Option<String>,
}

impl NewTask {
    /// Create a task input with the default `auto` workflow.
    pub fn new(project_id: impl Into<String>, title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            title: title.into(),
            description: description.into(),
            workflow: "auto".to_string(),
            ..Default::default()
        }
    }
}

/// Task storage contract. Implemented by the SQLite repository.
pub trait TaskRepository {
    fn create_task(&self, new: &NewTask) -> NodkrayResult<Task>;
    fn get_task(&self, task_id: &str) -> NodkrayResult<Option<Task>>;
    /// Update the status and append a `task_events` row in one transaction.
    fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        event: &str,
        payload: Option<serde_json::Value>,
    ) -> NodkrayResult<Task>;
    fn list_tasks(
        &self,
        project_id: Option<&str>,
        include_terminal: bool,
    ) -> NodkrayResult<Vec<TaskSummary>>;
    fn list_task_events(&self, task_id: &str) -> NodkrayResult<Vec<TaskEvent>>;
    /// Append an event without changing the status (worker lifecycle, §78).
    fn append_event(
        &self,
        task_id: &str,
        event: &str,
        payload: Option<serde_json::Value>,
    ) -> NodkrayResult<()>;
}

/// Create a task via any repository.
pub fn create_task(repo: &dyn TaskRepository, new: &NewTask) -> NodkrayResult<Task> {
    if new.title.trim().is_empty() {
        return Err(NodkrayError::user_input(
            "TASK_TITLE_REQUIRED",
            "a task title is required",
        ));
    }
    repo.create_task(new)
}

/// Fetch a task by id.
pub fn get_task(repo: &dyn TaskRepository, task_id: &str) -> NodkrayResult<Option<Task>> {
    repo.get_task(task_id)
}

/// Change a task status, recording the event atomically.
pub fn update_task_status(
    repo: &dyn TaskRepository,
    task_id: &str,
    status: TaskStatus,
    event: &str,
    payload: Option<serde_json::Value>,
) -> NodkrayResult<Task> {
    repo.update_task_status(task_id, status, event, payload)
}

/// List tasks for a project (or all projects when `project_id` is `None`).
pub fn list_tasks(
    repo: &dyn TaskRepository,
    project_id: Option<&str>,
    include_terminal: bool,
) -> NodkrayResult<Vec<TaskSummary>> {
    repo.list_tasks(project_id, include_terminal)
}

/// Active (non-terminal) tasks of a project.
pub fn active_tasks(repo: &dyn TaskRepository, project_id: &str) -> NodkrayResult<Vec<TaskSummary>> {
    repo.list_tasks(Some(project_id), false)
}

/// Append a task event.
pub fn append_event(
    repo: &dyn TaskRepository,
    task_id: &str,
    event: &str,
    payload: Option<serde_json::Value>,
) -> NodkrayResult<()> {
    repo.append_event(task_id, event, payload)
}

/// Cancel a task: mark it `CANCELLED` and best-effort terminate its worker.
pub fn cancel_task(
    repo: &crate::memory::sqlite::SqliteMemoryRepository,
    task_id: &str,
) -> NodkrayResult<Task> {
    let task = repo.get_task(task_id)?.ok_or_else(|| {
        NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task {task_id}"))
    })?;
    if task.status.is_terminal() {
        return Err(NodkrayError::user_input(
            "TASK_ALREADY_TERMINAL",
            format!("task {task_id} is already {}", task.status.as_str()),
        ));
    }

    if let Some(pid) = repo.latest_worker_pid(task_id)? {
        if crate::memory::process_alive(pid) {
            // Best effort: the worker may already be gone.
            let _ = std::process::Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status();
            tracing::warn!(pid, "sent SIGTERM to worker process");
        }
    }

    update_task_status(
        repo,
        task_id,
        TaskStatus::Cancelled,
        "task.cancelled",
        Some(serde_json::json!({ "reason": "user requested" })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::sqlite::SqliteMemoryRepository;
    use crate::memory::MemoryRepository;

    fn repo() -> SqliteMemoryRepository {
        SqliteMemoryRepository::open_in_memory().expect("repo")
    }

    #[test]
    fn create_task_starts_pending_and_records_creation_event() {
        let repo = repo();
        let project = repo.get_or_create_project("/tmp/tasks").expect("project");
        let task = create_task(&repo, &NewTask::new(project.id, "title", "desc")).expect("create");

        assert!(task.id.starts_with("task_"));
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.workflow, "auto");
        let events = repo.list_task_events(&task.id).expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "task.created");
    }

    #[test]
    fn status_update_records_events_in_one_transaction() {
        let repo = repo();
        let project = repo.get_or_create_project("/tmp/tasks").expect("project");
        let task = create_task(&repo, &NewTask::new(project.id, "t", "d")).expect("create");

        update_task_status(&repo, &task.id, TaskStatus::Classifying, "task.classifying", None)
            .expect("classify");
        let updated = update_task_status(
            &repo,
            &task.id,
            TaskStatus::Running,
            "task.running",
            Some(serde_json::json!({ "worker": "w1" })),
        )
        .expect("run");

        assert_eq!(updated.status, TaskStatus::Running);
        let events = repo.list_task_events(&task.id).expect("events");
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event, "task.running");
        assert_eq!(
            events[2]
                .payload
                .as_ref()
                .and_then(|value| value.get("worker"))
                .and_then(|value| value.as_str()),
            Some("w1")
        );
    }

    #[test]
    fn updating_unknown_task_fails_without_partial_write() {
        let repo = repo();
        let err = update_task_status(&repo, "task_missing", TaskStatus::Running, "task.running", None)
            .expect_err("must fail");
        assert_eq!(err.code(), "TASK_NOT_FOUND");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn task_survives_reopen_like_a_crash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("memory.db");

        let task_id = {
            let repo = SqliteMemoryRepository::open(&path).expect("open");
            let project = repo.get_or_create_project("/tmp/crash").expect("project");
            let task = create_task(&repo, &NewTask::new(project.id, "survivor", "desc")).expect("create");
            update_task_status(&repo, &task.id, TaskStatus::Reviewing, "task.reviewing", None)
                .expect("update");
            task.id
            // repo dropped here == process killed
        };

        let repo = SqliteMemoryRepository::open(&path).expect("reopen");
        let recovered = get_task(&repo, &task_id).expect("get").expect("present");
        assert_eq!(recovered.status, TaskStatus::Reviewing);
        assert_eq!(repo.list_task_events(&task_id).expect("events").len(), 2);
    }

    #[test]
    fn active_list_excludes_terminal_states() {
        let repo = repo();
        let project = repo.get_or_create_project("/tmp/tasks").expect("project");
        let open = create_task(&repo, &NewTask::new(project.id.clone(), "open", "d")).expect("a");
        let done = create_task(&repo, &NewTask::new(project.id.clone(), "done", "d")).expect("b");
        update_task_status(&repo, &done.id, TaskStatus::Passed, "task.passed", None).expect("pass");

        let active = active_tasks(&repo, &project.id).expect("active");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, open.id);

        let all = list_tasks(&repo, Some(&project.id), true).expect("all");
        assert_eq!(all.len(), 2);
    }
}
