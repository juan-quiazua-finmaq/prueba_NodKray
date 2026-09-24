//! `nodkray task` (spec §17, §21, §140). Phase 2 runs ST end-to-end.

use clap::{Args, Subcommand};
use serde_json::json;

use crate::cli::Context;
use crate::core::task::{self, TaskRepository};
use crate::core::workflow::{make_title, require_description, run_st, StRun, WorkflowRequest};
use crate::error::{NodkrayError, NodkrayResult};

/// Arguments for `nodkray task`.
#[derive(Debug, Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: Option<TaskCommand>,
    #[command(flatten)]
    pub flags: RunFlags,
    /// Task description (when running a task directly, without a subcommand).
    #[arg(value_name = "DESCRIPTION", trailing_var_arg = true, allow_hyphen_values = true)]
    pub text: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// Create and run a task.
    Run(RunArgs),
    /// Show a persisted task and its event trail.
    Inspect(InspectArgs),
    /// Cancel a task (best effort).
    Cancel(CancelArgs),
}

/// Flags shared by `nodkray task ...` and `nodkray task run ...`.
#[derive(Debug, Clone, Default, Args)]
pub struct RunFlags {
    /// Force a workflow; only `st` is implemented.
    #[arg(long)]
    pub workflow: Option<String>,
    /// Review depth; only `fast` is implemented.
    #[arg(long)]
    pub review: Option<String>,
    /// Relax worker permissions (never changes the workflow).
    #[arg(long)]
    pub yolo: bool,
    /// Task description (alternative to the positional argument).
    #[arg(long)]
    pub description: Option<String>,
}

/// Arguments for `nodkray task run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub flags: RunFlags,
    /// Task description.
    #[arg(value_name = "DESCRIPTION", trailing_var_arg = true, allow_hyphen_values = true)]
    pub text: Vec<String>,
}

/// Arguments for `nodkray task inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Task id (`task_<ulid>`).
    pub id: String,
}

/// Arguments for `nodkray task cancel`.
#[derive(Debug, Args)]
pub struct CancelArgs {
    /// Task id (`task_<ulid>`).
    pub id: String,
}

/// Run `nodkray task`.
pub fn run(ctx: &Context, args: TaskArgs) -> NodkrayResult<i32> {
    match args.command {
        Some(TaskCommand::Run(run_args)) => {
            let description = resolve_description(&run_args.flags, &run_args.text);
            execute(ctx, &run_args.flags, description)
        }
        Some(TaskCommand::Inspect(inspect_args)) => inspect(ctx, inspect_args),
        Some(TaskCommand::Cancel(cancel_args)) => cancel(ctx, cancel_args),
        None => {
            let description = resolve_description(&args.flags, &args.text);
            execute(ctx, &args.flags, description)
        }
    }
}

fn resolve_description(flags: &RunFlags, positional: &[String]) -> String {
    flags
        .description
        .clone()
        .or_else(|| (!positional.is_empty()).then(|| positional.join(" ")))
        .unwrap_or_default()
}

fn execute(ctx: &Context, flags: &RunFlags, description: String) -> NodkrayResult<i32> {
    require_description(&description)?;

    if let Some(workflow) = flags.workflow.as_deref() {
        if workflow != "st" {
            return Err(NodkrayError::user_input(
                "WORKFLOW_NOT_IMPLEMENTED",
                format!("workflow `{workflow}` is not implemented (only `st`)"),
            ));
        }
    }
    if let Some(depth) = flags.review.as_deref() {
        if depth != "fast" {
            return Err(NodkrayError::user_input(
                "REVIEW_DEPTH_NOT_IMPLEMENTED",
                format!(
                    "review depth `{depth}` is not implemented in phase 2 (only `fast`)"
                ),
            ));
        }
    }

    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;
    let project_context = ctx.project_context()?;

    let request = WorkflowRequest {
        title: make_title(&description),
        description,
        force_workflow: flags.workflow.clone(),
        review_override: flags.review.clone(),
        yolo: flags.yolo,
    };
    let outcome = run_st(&StRun {
        repo: &repo,
        config: &project_context.config,
        project: &project,
        root: &project_context.root,
        request,
    })?;

    for warning in &outcome.warnings {
        ctx.output.warn(warning);
    }
    ctx.output.emit_json(&outcome);
    ctx.output.emit_text(format!(
        "task {} [{}] workflow={} effort={}",
        outcome.task_id, outcome.status, outcome.workflow, outcome.effort
    ));
    for worker in &outcome.workers {
        ctx.output.emit_text(format!(
            "worker: {} ({}) -> {}",
            worker.role, worker.agent, worker.status
        ));
    }
    ctx.output.emit_text(format!(
        "review: {} ({})",
        outcome.review.status, outcome.review.depth
    ));
    ctx.output.emit_text(format!("merge: {}", outcome.merge.status));

    Ok(outcome.exit_code)
}

fn inspect(ctx: &Context, args: InspectArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let task = task::get_task(&repo, &args.id)?.ok_or_else(|| {
        NodkrayError::user_input("TASK_NOT_FOUND", format!("unknown task id `{}`", args.id))
    })?;
    let events = repo.list_task_events(&args.id)?;

    if ctx.output.json {
        ctx.output.emit_json(&json!({ "task": task, "events": events }));
    } else {
        ctx.output.emit_text(format!(
            "{}  [{}]  workflow={}",
            task.id,
            task.status.as_str(),
            task.workflow
        ));
        ctx.output.emit_text(format!("title: {}", task.title));
        ctx.output.emit_text(format!("project: {}", task.project_id));
        ctx.output
            .emit_text(format!("created: {}  updated: {}", task.created_at, task.updated_at));
        if !task.description.is_empty() {
            ctx.output.emit_text(format!("description: {}", task.description));
        }
        ctx.output.emit_text("events:");
        for event in &events {
            ctx.output
                .emit_text(format!("  {}  {}", event.created_at, event.event));
        }
    }

    Ok(0)
}

fn cancel(ctx: &Context, args: CancelArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let task = task::cancel_task(&repo, &args.id)?;

    ctx.output
        .emit_json(&json!({ "id": task.id, "status": task.status.as_str() }));
    ctx.output
        .emit_text(format!("{} [{}]", task.id, task.status.as_str()));
    Ok(0)
}
