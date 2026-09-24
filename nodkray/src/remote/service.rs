//! Control-API service. The HTTP layer only translates requests; every
//! mutation goes through existing core/memory functions (spec §86).

use std::path::{Path, PathBuf};
use std::thread;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{load_effective, Config, ConfigPaths};
use crate::core::decision::{self, DecisionInput};
use crate::core::project::resolve_project;
use crate::core::task::{self, NewTask, Task, TaskEvent, TaskRepository, TaskSummary};
use crate::core::versions::snapshot_session;
use crate::core::workflow::{make_title, require_description, run_st, StRun, WorkflowRequest};
use crate::error::{NodkrayError, NodkrayResult};
use crate::memory::sqlite::SqliteMemoryRepository;
use crate::memory::{NewSession, Review, Worker};

/// Status payload for `GET /status`.
#[derive(Debug, Clone, Serialize)]
pub struct StatusPayload {
    pub ok: bool,
    pub tasks: Vec<TaskSummary>,
}

/// Body for `POST /tasks`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CreateTaskRequest {
    pub description: String,
    #[serde(default)]
    pub workflow: Option<String>,
    #[serde(default)]
    pub review: Option<String>,
    #[serde(default)]
    pub yolo: bool,
    /// When true, spawn the workflow in a background thread after persist.
    #[serde(default)]
    pub execute: bool,
}

/// Result of creating a task through the control API.
#[derive(Debug, Clone, Serialize)]
pub struct CreateTaskResponse {
    pub task: Task,
    pub workflow: String,
    pub effort: u32,
    pub session_id: String,
    pub executing: bool,
}

/// Task + events for `GET /tasks/:id`.
#[derive(Debug, Clone, Serialize)]
pub struct TaskPayload {
    pub task: Task,
    pub events: Vec<TaskEvent>,
}

/// Core operations the Control API may perform.
pub trait ControlService: Send + Sync {
    fn status(&self) -> NodkrayResult<StatusPayload>;
    fn create_task(&self, request: CreateTaskRequest) -> NodkrayResult<CreateTaskResponse>;
    fn get_task(&self, task_id: &str) -> NodkrayResult<TaskPayload>;
    fn list_workers(&self) -> NodkrayResult<Vec<Worker>>;
    fn list_reviews(&self) -> NodkrayResult<Vec<Review>>;
    fn cancel_task(&self, task_id: &str) -> NodkrayResult<Task>;
    fn events(&self, limit: usize) -> NodkrayResult<Vec<TaskEvent>>;
}

/// Production service: opens SQLite per call and delegates to core.
pub struct CoreControlService {
    paths: ConfigPaths,
    cwd: PathBuf,
}

impl CoreControlService {
    pub fn new(paths: ConfigPaths, cwd: PathBuf) -> Self {
        Self { paths, cwd }
    }

    fn open(&self) -> NodkrayResult<(SqliteMemoryRepository, Config, crate::memory::Project, PathBuf)>
    {
        let root = crate::core::project::find_project_root(&self.cwd).root;
        let config = load_effective(&self.paths, Some(&root))?;
        let db_path = self.paths.resolve_memory_path(&config.memory.path);
        let repo = SqliteMemoryRepository::open(&db_path)?;
        let project = resolve_project(&repo, &root)?;
        Ok((repo, config, project, root))
    }
}

impl ControlService for CoreControlService {
    fn status(&self) -> NodkrayResult<StatusPayload> {
        let (repo, _config, project, _root) = self.open()?;
        let tasks = task::active_tasks(&repo, &project.id)?;
        Ok(StatusPayload { ok: true, tasks })
    }

    fn create_task(&self, request: CreateTaskRequest) -> NodkrayResult<CreateTaskResponse> {
        require_description(&request.description)?;
        let (repo, config, project, root) = self.open()?;
        let title = make_title(&request.description);
        let decision = decision::classify(
            &config.decision,
            &DecisionInput {
                title: title.clone(),
                description: request.description.clone(),
            },
            request.workflow.as_deref() == Some("st"),
        )?;

        let session = repo.create_session(&NewSession {
            project_id: project.id.clone(),
            frontier_agent: Some(config.agents.frontier.provider.clone()),
            workflow: Some(decision.workflow.clone()),
            status: "running".to_string(),
            config_snapshot: Some(snapshot_session(&config, &root)),
        })?;

        let task = task::create_task(
            &repo,
            &NewTask {
                project_id: project.id.clone(),
                session_id: Some(session.id.clone()),
                title,
                description: request.description.clone(),
                workflow: decision.workflow.to_ascii_lowercase(),
                effort: Some(i64::from(decision.effort)),
                role: Some("default".to_string()),
                parent_task_id: None,
            },
        )?;

        let executing = request.execute;
        if executing {
            spawn_execution(
                self.paths.clone(),
                self.cwd.clone(),
                request,
                session.id.clone(),
                task.id.clone(),
            );
        }

        Ok(CreateTaskResponse {
            task,
            workflow: decision.workflow,
            effort: decision.effort,
            session_id: session.id,
            executing,
        })
    }

    fn get_task(&self, task_id: &str) -> NodkrayResult<TaskPayload> {
        let (repo, _config, _project, _root) = self.open()?;
        let task = task::get_task(&repo, task_id)?.ok_or_else(|| {
            NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task id `{task_id}`"))
        })?;
        let events = repo.list_task_events(task_id)?;
        Ok(TaskPayload { task, events })
    }

    fn list_workers(&self) -> NodkrayResult<Vec<Worker>> {
        let (repo, _config, project, _root) = self.open()?;
        repo.list_workers(Some(&project.id))
    }

    fn list_reviews(&self) -> NodkrayResult<Vec<Review>> {
        let (repo, _config, project, _root) = self.open()?;
        repo.list_reviews(Some(&project.id))
    }

    fn cancel_task(&self, task_id: &str) -> NodkrayResult<Task> {
        let (repo, _config, _project, _root) = self.open()?;
        task::cancel_task(&repo, task_id)
    }

    fn events(&self, limit: usize) -> NodkrayResult<Vec<TaskEvent>> {
        let (repo, _config, _project, _root) = self.open()?;
        repo.list_recent_events(limit.max(1).min(500))
    }
}

fn spawn_execution(
    paths: ConfigPaths,
    cwd: PathBuf,
    request: CreateTaskRequest,
    session_id: String,
    task_id: String,
) {
    thread::spawn(move || {
        if let Err(error) = run_existing(&paths, &cwd, &request, &session_id, &task_id) {
            tracing::error!(
                task_id,
                code = error.code(),
                message = error.message(),
                "background control-API task failed"
            );
        }
    });
}

fn run_existing(
    paths: &ConfigPaths,
    cwd: &Path,
    request: &CreateTaskRequest,
    session_id: &str,
    task_id: &str,
) -> NodkrayResult<()> {
    let root = crate::core::project::find_project_root(cwd).root;
    let config = load_effective(paths, Some(&root))?;
    let db_path = paths.resolve_memory_path(&config.memory.path);
    let repo = SqliteMemoryRepository::open(&db_path)?;
    let project = resolve_project(&repo, &root)?;
    let _ = run_st(&StRun {
        repo: &repo,
        config: &config,
        project: &project,
        root: &root,
        request: WorkflowRequest {
            title: make_title(&request.description),
            description: request.description.clone(),
            force_workflow: request.workflow.clone(),
            review_override: request.review.clone(),
            yolo: request.yolo,
            resume_session_id: Some(session_id.to_string()),
            resume_task_id: Some(task_id.to_string()),
        },
    })?;
    Ok(())
}

/// In-memory control service used by HTTP tests (no SQLite, no workflow).
#[derive(Default)]
pub struct MemoryControlService {
    pub tasks: std::sync::Mutex<Vec<Task>>,
    pub events: std::sync::Mutex<Vec<TaskEvent>>,
    pub workers: std::sync::Mutex<Vec<Worker>>,
    pub reviews: std::sync::Mutex<Vec<Review>>,
}

impl MemoryControlService {
    pub fn seed_task(&self, task: Task) {
        self.tasks.lock().expect("lock").push(task);
    }
}

impl ControlService for MemoryControlService {
    fn status(&self) -> NodkrayResult<StatusPayload> {
        let tasks = self.tasks.lock().expect("lock");
        Ok(StatusPayload {
            ok: true,
            tasks: tasks
                .iter()
                .map(|task| TaskSummary {
                    id: task.id.clone(),
                    title: task.title.clone(),
                    status: task.status,
                    workflow: task.workflow.clone(),
                    updated_at: task.updated_at.clone(),
                })
                .collect(),
        })
    }

    fn create_task(&self, request: CreateTaskRequest) -> NodkrayResult<CreateTaskResponse> {
        require_description(&request.description)?;
        let id = format!("task_{}", self.tasks.lock().expect("lock").len() + 1);
        let task = Task {
            id: id.clone(),
            project_id: "project_test".to_string(),
            session_id: Some("session_test".to_string()),
            parent_task_id: None,
            title: make_title(&request.description),
            description: request.description,
            workflow: request.workflow.unwrap_or_else(|| "st".to_string()),
            effort: Some(5),
            role: Some("default".to_string()),
            status: crate::core::task::TaskStatus::Pending,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        self.tasks.lock().expect("lock").push(task.clone());
        Ok(CreateTaskResponse {
            task,
            workflow: "ST".to_string(),
            effort: 5,
            session_id: "session_test".to_string(),
            executing: request.execute,
        })
    }

    fn get_task(&self, task_id: &str) -> NodkrayResult<TaskPayload> {
        let task = self
            .tasks
            .lock()
            .expect("lock")
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
            .ok_or_else(|| {
                NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task id `{task_id}`"))
            })?;
        let events = self
            .events
            .lock()
            .expect("lock")
            .iter()
            .filter(|event| event.task_id == task_id)
            .cloned()
            .collect();
        Ok(TaskPayload { task, events })
    }

    fn list_workers(&self) -> NodkrayResult<Vec<Worker>> {
        Ok(self.workers.lock().expect("lock").clone())
    }

    fn list_reviews(&self) -> NodkrayResult<Vec<Review>> {
        Ok(self.reviews.lock().expect("lock").clone())
    }

    fn cancel_task(&self, task_id: &str) -> NodkrayResult<Task> {
        let mut tasks = self.tasks.lock().expect("lock");
        let task = tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| {
                NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task id `{task_id}`"))
            })?;
        task.status = crate::core::task::TaskStatus::Cancelled;
        Ok(task.clone())
    }

    fn events(&self, _limit: usize) -> NodkrayResult<Vec<TaskEvent>> {
        Ok(self.events.lock().expect("lock").clone())
    }
}

/// Helper so unused Value imports stay available for future payloads.
#[allow(dead_code)]
fn _value_ok() -> Value {
    Value::Null
}
