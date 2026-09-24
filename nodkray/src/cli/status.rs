//! `nodkray status` (spec §17, §110): active tasks of the current project,
//! read from SQLite. `RUNNING` tasks with no live worker are flagged stale.

use clap::Args;
use serde::Serialize;

use crate::cli::Context;
use crate::core::task::{self, TaskStatus};
use crate::error::NodkrayResult;

/// Arguments for `nodkray status`.
#[derive(Debug, Args)]
pub struct StatusArgs {}

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
pub fn run(ctx: &Context, _args: StatusArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;

    let mut tasks = Vec::new();
    for summary in task::active_tasks(&repo, &project.id)? {
        let stale = summary.status == TaskStatus::Running
            && match repo.latest_worker_pid(&summary.id)? {
                Some(pid) => !crate::memory::process_alive(pid),
                None => true,
            };
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
        ctx.output.emit_text("no active tasks");
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
