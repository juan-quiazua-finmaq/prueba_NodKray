//! `nodkray status` (spec §17, §110): active tasks of the current project,
//! read from SQLite. `RUNNING` tasks with no live worker are flagged stale.
//! `CLASSIFYING` / `PLANNED` tasks whose session has ended are also stale.
//! Finished tasks are hidden unless `--all`.

use clap::Args;
use serde::Serialize;

use crate::cli::Context;
use crate::core::task::{self, TaskStatus};
use crate::error::NodkrayResult;

/// Arguments for `nodkray status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Include terminal tasks (PASSED/FAILED/MERGED/…). Default lists only active ones.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Serialize)]
struct StatusTask {
    id: String,
    title: String,
    status: TaskStatus,
    workflow: String,
    updated_at: String,
    stale: bool,
}

#[derive(Debug, Serialize)]
struct StatusReport {
    tasks: Vec<StatusTask>,
}

/// Run `nodkray status`.
pub fn run(ctx: &Context, args: StatusArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;

    let summaries = task::list_tasks(&repo, Some(&project.id), args.all)?;
    let mut tasks = Vec::new();
    for summary in summaries {
        let stale = is_stale(&repo, &summary)?;
        tasks.push(StatusTask {
            id: summary.id,
            title: summary.title,
            status: summary.status,
            workflow: summary.workflow,
            updated_at: summary.updated_at,
            stale,
        });
    }

    let report = StatusReport { tasks };

    if ctx.output.json {
        ctx.output.emit_json(&report);
    } else if report.tasks.is_empty() {
        if args.all {
            ctx.output.emit_text("no tasks");
        } else {
            ctx.output.emit_text("no active tasks (use `nodkray status --all` for finished ones)");
        }
    } else {
        for task in &report.tasks {
            let stale = if task.stale { " (stale)" } else { "" };
            ctx.output.emit_text(format!(
                "{}  [{}]{}  {}  ({})",
                task.id,
                task.status.as_str(),
                stale,
                task.title,
                task.workflow
            ));
        }
    }

    Ok(0)
}

fn is_stale(
    repo: &crate::memory::sqlite::SqliteMemoryRepository,
    summary: &crate::core::task::TaskSummary,
) -> NodkrayResult<bool> {
    match summary.status {
        TaskStatus::Running => Ok(match repo.latest_worker_pid(&summary.id)? {
            Some(pid) => !crate::memory::process_alive(pid),
            None => true,
        }),
        TaskStatus::Classifying | TaskStatus::Planned | TaskStatus::Pending => {
            let Some(task) = task::get_task(repo, &summary.id)? else {
                return Ok(true);
            };
            let Some(session_id) = task.session_id else {
                return Ok(true);
            };
            Ok(match repo.session(&session_id)? {
                Some(session) => session.ended_at.is_some() || session.status != "running",
                None => true,
            })
        }
        _ => Ok(false),
    }
}
