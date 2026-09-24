//! ST workflow (spec §21): init → classify → execute → review → finalize.
//!
//! Every state transition is persisted together with a `task_events` row; a
//! crash leaves the task in its last recorded state (spec §110).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;

use crate::agents::{AgentOutput, AgentRegistry, AgentRequest, ProcessSpec};
use crate::config::Config;
use crate::core::decision::{self, DecisionInput};
use crate::core::roles;
use crate::core::task::{self, NewTask, TaskRepository, TaskStatus};
use crate::core::workflow::depth::{select_review_depth, ReviewDepth, WorkflowKind};
use crate::core::workflow::odd::{write_task_md, OddTask};
use crate::error::{NodkrayError, NodkrayResult};
use crate::execution::{
    select_backend, WorkerEvent, WorkerSpec, WorkerStatus, WorkspaceRequest,
};
use crate::memory::sqlite::SqliteMemoryRepository;
use crate::memory::{MemoryRepository, NewDecision, NewSession, NewWorker, Project};
use crate::review::engine::{run_review, ReviewRequest};
use crate::review::git::{self, MergeStatus};

/// Default test timeout for RDD FAST (spec §31): 10 minutes.
pub const TEST_TIMEOUT: Duration = Duration::from_secs(600);

/// What the caller asks the ST workflow to do.
#[derive(Debug, Clone, Default)]
pub struct WorkflowRequest {
    pub title: String,
    pub description: String,
    /// Forced workflow: `st`, `odd`, `sdd`, or `auto`.
    pub force_workflow: Option<String>,
    /// Requested review depth: `fast`, `balanced`, or `deep`.
    pub review_override: Option<String>,
    pub yolo: bool,
    /// Resume a session already created by the control API.
    pub resume_session_id: Option<String>,
    /// Resume a task already created by the control API.
    pub resume_task_id: Option<String>,
}

/// Inputs and collaborators for one ST run.
pub struct StRun<'a> {
    pub repo: &'a SqliteMemoryRepository,
    pub config: &'a Config,
    pub project: &'a Project,
    pub root: &'a Path,
    pub request: WorkflowRequest,
}

/// A worker as reported in the final summary (spec §140).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkerSummary {
    pub role: String,
    pub agent: String,
    pub status: String,
}

/// Review summary (spec §140).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewSummary {
    pub depth: String,
    pub status: String,
}

/// Merge summary (spec §140).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeSummary {
    pub status: String,
}

/// Structured result of a run (spec §140). `exit_code`/`warnings` are not part
/// of the JSON document.
#[derive(Debug, Clone, Serialize)]
pub struct StOutcome {
    pub task_id: String,
    pub workflow: String,
    pub effort: u32,
    pub status: String,
    pub workers: Vec<WorkerSummary>,
    pub review: ReviewSummary,
    pub merge: MergeSummary,
    #[serde(skip_serializing)]
    pub exit_code: i32,
    #[serde(skip_serializing)]
    pub warnings: Vec<String>,
}

/// Run the ST pipeline.
pub fn run_st(run: &StRun<'_>) -> NodkrayResult<StOutcome> {
    let repo = run.repo;
    let config = run.config;
    let project = run.project;
    let root = run.root;
    let request = &run.request;

    let base_branch = git::current_branch(root)?.unwrap_or_else(|| "main".to_string());

    // ---- init: session with a config + versions snapshot (spec §127, §164) ----
    let (session, task) = match (&request.resume_session_id, &request.resume_task_id) {
        (Some(session_id), Some(task_id)) => {
            let session = repo.session(session_id)?.ok_or_else(|| {
                NodkrayError::memory(
                    "SESSION_NOT_FOUND",
                    format!("unknown session {session_id}"),
                )
            })?;
            let task = task::get_task(repo, task_id)?.ok_or_else(|| {
                NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task {task_id}"))
            })?;
            (session, task)
        }
        _ => {
            let session = repo.create_session(&NewSession {
                project_id: project.id.clone(),
                frontier_agent: Some(config.agents.frontier.provider.clone()),
                workflow: Some("auto".to_string()),
                status: "running".to_string(),
                config_snapshot: Some(crate::core::versions::snapshot_session(config, root)),
            })?;
            let task = task::create_task(
                repo,
                &NewTask {
                    project_id: project.id.clone(),
                    session_id: Some(session.id.clone()),
                    title: request.title.clone(),
                    description: request.description.clone(),
                    workflow: "auto".to_string(),
                    effort: None,
                    role: Some("default".to_string()),
                    parent_task_id: None,
                },
            )?;
            (session, task)
        }
    };
    task::update_task_status(repo, &task.id, TaskStatus::Classifying, "task.classifying", None)?;

    // ---- classify ----
    let decision_input = DecisionInput {
        title: request.title.clone(),
        description: request.description.clone(),
    };
    let force_kind = request
        .force_workflow
        .as_deref()
        .filter(|value| *value != "auto")
        .and_then(WorkflowKind::parse);
    let force_st = force_kind == Some(WorkflowKind::St);
    let mut warnings: Vec<String> = Vec::new();

    let mut decision = match decision::classify(&config.decision, &decision_input, force_st) {
        Ok(decision) => decision,
        Err(error) => {
            task::update_task_status(
                repo,
                &task.id,
                TaskStatus::Blocked,
                "task.blocked",
                Some(json!({ "reason": error.message() })),
            )?;
            let _ = repo.end_session(&session.id, "blocked");
            return Err(error);
        }
    };

    if let Some(kind) = force_kind {
        if kind != WorkflowKind::St {
            decision.workflow = kind.as_str().to_string();
        }
    }

    if force_st && decision.effort > config.decision.thresholds.st_max {
        warnings.push(decision::force_st_warning(
            decision.effort,
            config.decision.thresholds.st_max,
        ));
    }

    let workflow = WorkflowKind::parse(&decision.workflow).unwrap_or(WorkflowKind::St);
    let review_override = request
        .review_override
        .as_deref()
        .and_then(ReviewDepth::parse);
    let review_depth = if review_override.is_some() || force_kind.is_none() {
        select_review_depth(
            workflow,
            decision.effort,
            &config.review.thresholds,
            review_override,
        )
    } else {
        workflow.default_depth()
    };
    decision.review_depth = review_depth.as_str().to_string();

    if workflow == WorkflowKind::Odd {
        write_task_md(
            root,
            &task.id,
            &OddTask::from_description(&request.description),
        )?;
    }
    let sdd_plan = if workflow == WorkflowKind::Sdd {
        Some(crate::core::workflow::sdd::prepare(root, false, &[])?)
    } else {
        None
    };

    repo.set_task_classification(
        &task.id,
        &decision.workflow.to_ascii_lowercase(),
        i64::from(decision.effort),
    )?;
    repo.save_decision(&NewDecision {
        project_id: project.id.clone(),
        task_id: Some(task.id.clone()),
        question: request.title.clone(),
        decision: format!(
            "workflow={}; effort={}; review_depth={}",
            decision.workflow, decision.effort, decision.review_depth
        ),
        rationale: Some(decision.reasons.join("; ")),
    })?;
    task::update_task_status(
        repo,
        &task.id,
        TaskStatus::Planned,
        "task.planned",
        Some(json!({
            "workflow": decision.workflow,
            "effort": decision.effort,
            "review_depth": decision.review_depth
        })),
    )?;

    // ---- execute ----
    task::update_task_status(
        repo,
        &task.id,
        TaskStatus::Running,
        "task.running",
        Some(json!({ "role": "default" })),
    )?;

    let role = "default";
    let provider = roles::worker_provider(config, role).to_string();
    let registry = AgentRegistry::with_defaults(config.agents.generic.command.clone());
    let adapter = registry.select(&provider)?;
    let reviewer_available = registry
        .select(roles::worker_provider(config, "reviewer"))
        .map(|reviewer| reviewer.detect().found)
        .unwrap_or(false);

    let backend = select_backend(
        &config.execution.backend,
        config.execution.fallback_console,
    )?;
    let workspace = if config.execution.worktrees.enabled {
        Some(backend.create_workspace(&WorkspaceRequest {
            root: root.to_path_buf(),
            task_id: task.id.clone(),
            token: role.to_string(),
            base_branch: Some(base_branch.clone()),
        })?)
    } else {
        None
    };
    let cwd: PathBuf = workspace
        .as_ref()
        .map(|ws| ws.path.clone())
        .unwrap_or_else(|| root.to_path_buf());

    let memory_context = gather_memory_context(repo, &project.id, &request.description);
    let agent_request = AgentRequest {
        task_id: task.id.clone(),
        project_root: root.to_path_buf(),
        workflow: decision.workflow.clone(),
        role: role.to_string(),
        description: request.description.clone(),
        constraints: Vec::new(),
        memory_context,
        yolo: request.yolo,
    };
    if let Some(plan) = &sdd_plan {
        for stage in &plan.stages {
            let stage_spec = WorkerSpec {
                task_id: task.id.clone(),
                role: "sdd".to_string(),
                agent: "speckit".to_string(),
                worktree: workspace.as_ref().map(|ws| ws.path.clone()),
                branch: workspace.as_ref().map(|ws| ws.branch.clone()),
                process: ProcessSpec {
                    program: stage.program.clone(),
                    args: stage.args.clone(),
                    stdin: None,
                    env: Vec::new(),
                },
            };
            let mut sink = |event: WorkerEvent| {
                let _ = repo.append_event(&task.id, &event.event, Some(event.payload));
            };
            let stage_outcome = backend.spawn_worker(&stage_spec, &cwd, &mut sink)?;
            repo.append_event(
                &task.id,
                "sdd.stage",
                Some(json!({
                    "stage": stage.stage,
                    "status": stage_outcome.status.as_str(),
                    "exit_code": stage_outcome.exit_code,
                })),
            )?;
            if stage_outcome.status == WorkerStatus::Failed {
                task::update_task_status(
                    repo,
                    &task.id,
                    TaskStatus::Failed,
                    "task.failed",
                    Some(json!({ "stage": stage.stage })),
                )?;
                let _ = repo.end_session(&session.id, "failed");
                return Err(NodkrayError::execution(
                    "SDD_STAGE_FAILED",
                    format!("Spec-Kit stage `{}` failed", stage.stage),
                ));
            }
        }
    }

    let process = adapter.non_interactive_command(&agent_request)?;

    let worker = repo.create_worker(&NewWorker {
        task_id: task.id.clone(),
        role: role.to_string(),
        agent: adapter.id().to_string(),
        worktree_path: Some(cwd.display().to_string()),
        status: "created".to_string(),
    })?;

    let worker_spec = WorkerSpec {
        task_id: task.id.clone(),
        role: role.to_string(),
        agent: adapter.id().to_string(),
        worktree: workspace.as_ref().map(|ws| ws.path.clone()),
        branch: workspace.as_ref().map(|ws| ws.branch.clone()),
        process,
    };

    let mut sink = |event: WorkerEvent| {
        let _ = repo.append_event(&task.id, &event.event, Some(event.payload));
    };
    let outcome = backend.spawn_worker(&worker_spec, &cwd, &mut sink)?;
    repo.update_worker_status(&worker.id, outcome.status.as_str())?;

    let parsed = adapter.parse_result(&AgentOutput {
        stdout: outcome.stdout.clone(),
        stderr: outcome.stderr.clone(),
        exit_code: outcome.exit_code,
    })?;
    if !parsed.parsed_json {
        warnings.push(format!(
            "{} did not emit Output Contract JSON; raw output preserved",
            adapter.id()
        ));
    }
    repo.append_event(
        &task.id,
        "worker.result",
        Some(json!({
            "status": parsed.result.status,
            "summary": parsed.result.summary,
            "parsed_json": parsed.parsed_json,
            "changed_files": parsed.result.changed_files,
        })),
    )?;

    let worker_summary = WorkerSummary {
        role: role.to_string(),
        agent: adapter.id().to_string(),
        status: outcome.status.as_str().to_string(),
    };

    // Agent failure must never become a successful task (spec §114).
    let blocked = parsed.result.status == "blocked";
    let failed = outcome.status == WorkerStatus::Failed || parsed.result.status == "failed";
    if failed || blocked {
        let status = if blocked {
            TaskStatus::Blocked
        } else {
            TaskStatus::Failed
        };
        task::update_task_status(
            repo,
            &task.id,
            status,
            if blocked { "task.blocked" } else { "task.failed" },
            Some(json!({ "summary": parsed.result.summary })),
        )?;
        let _ = repo.end_session(&session.id, &status.as_str().to_lowercase());
        return Ok(StOutcome {
            task_id: task.id,
            workflow: decision.workflow,
            effort: decision.effort,
            status: status.as_str().to_string(),
            workers: vec![worker_summary],
            review: ReviewSummary {
                depth: "none".to_string(),
                status: "skipped".to_string(),
            },
            merge: MergeSummary {
                status: "skipped".to_string(),
            },
            exit_code: if blocked { 8 } else { 5 },
            warnings,
        });
    }

    // ---- review ----
    task::update_task_status(repo, &task.id, TaskStatus::Reviewing, "task.reviewing", None)?;
    let review = run_review(
        repo,
        &ReviewRequest {
            task_id: task.id.clone(),
            depth: review_depth.as_str().to_string(),
            root: root.to_path_buf(),
            worktree: workspace.as_ref().map(|ws| ws.path.clone()),
            base_branch: base_branch.clone(),
            policy: config.review.policy.clone(),
            test_timeout: TEST_TIMEOUT,
            sentrux_enabled: config.integrations.sentrux.enabled,
            sentrux_required: config.integrations.sentrux.required,
            reviewer_available,
        },
    )?;
    let review_summary = ReviewSummary {
        depth: review_depth.as_str().to_string(),
        status: review.status.clone(),
    };

    if review.status != "passed" {
        let blocked = review.status == "blocked";
        let status = if blocked {
            TaskStatus::Blocked
        } else {
            TaskStatus::Failed
        };
        task::update_task_status(
            repo,
            &task.id,
            status,
            if blocked { "task.blocked" } else { "task.failed" },
            Some(json!({ "review_status": review.status })),
        )?;
        let _ = repo.end_session(&session.id, &status.as_str().to_lowercase());
        return Ok(StOutcome {
            task_id: task.id,
            workflow: decision.workflow,
            effort: decision.effort,
            status: status.as_str().to_string(),
            workers: vec![worker_summary],
            review: review_summary,
            merge: MergeSummary {
                status: "skipped".to_string(),
            },
            exit_code: if blocked { 8 } else { 7 },
            warnings,
        });
    }

    // ---- finalize: commit + merge ----
    let (merge_status, task_status, exit_code) = match &workspace {
        Some(workspace) => {
            git::commit_all(
                &workspace.path,
                &format!("nodkray: {} ({})", request.title, task.id),
            )?;
            let merge = git::merge_branch(root, &workspace.branch)?;
            match merge.status {
                MergeStatus::Merged => ("merged", TaskStatus::Merged, 0),
                MergeStatus::Conflict => ("conflict", TaskStatus::Conflict, 9),
                MergeStatus::Failed => ("failed", TaskStatus::Failed, 1),
            }
        }
        // No worktree: review passed on the main tree; do not claim a merge.
        None => ("skipped", TaskStatus::Passed, 0),
    };

    let event = match merge_status {
        "merged" => "task.merged",
        "conflict" => "task.conflict",
        "skipped" => "task.passed",
        _ => "task.failed",
    };
    task::update_task_status(
        repo,
        &task.id,
        task_status,
        event,
        Some(json!({ "merge": merge_status })),
    )?;
    let _ = repo.end_session(&session.id, &task_status.as_str().to_lowercase());

    Ok(StOutcome {
        task_id: task.id,
        workflow: decision.workflow,
        effort: decision.effort,
        status: task_status.as_str().to_string(),
        workers: vec![worker_summary],
        review: review_summary,
        merge: MergeSummary {
            status: merge_status.to_string(),
        },
        exit_code,
        warnings,
    })
}

/// Preview-only memory context for the worker prompt (spec §57, §133).
fn gather_memory_context(
    repo: &SqliteMemoryRepository,
    project_id: &str,
    description: &str,
) -> Vec<String> {
    let query = description
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return Vec::new();
    }
    match repo.search_memory(&query, Some(project_id), 5, 0) {
        Ok(results) => results
            .into_iter()
            .map(|preview| {
                format!(
                    "[{}] {}: {}",
                    preview.memory_type, preview.title, preview.preview
                )
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Convenience constructor used by the CLI and tests.
pub fn make_title(description: &str) -> String {
    let title: String = description
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    if title.is_empty() {
        return "untitled task".to_string();
    }
    if title.chars().count() <= 80 {
        title
    } else {
        title.chars().take(80).collect::<String>() + "…"
    }
}

/// Missing-config helper used by the CLI before it can build a [`StRun`].
pub fn require_description(description: &str) -> NodkrayResult<()> {
    if description.trim().is_empty() {
        return Err(NodkrayError::user_input(
            "TASK_DESCRIPTION_REQUIRED",
            "provide a task description",
        ));
    }
    Ok(())
}
