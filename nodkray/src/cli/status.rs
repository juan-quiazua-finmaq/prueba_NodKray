//! `nodkray status` (spec §17). Phase 0 reports no tasks but is valid JSON.

use clap::Args;
use serde::Serialize;

use crate::cli::Context;
use crate::core::task::{active_tasks, TaskSummary};
use crate::error::NodkrayResult;

/// Arguments for `nodkray status`.
#[derive(Debug, Args)]
pub struct StatusArgs {}

#[derive(Debug, Serialize)]
struct StatusReport {
    tasks: Vec<TaskSummary>,
}

/// Run `nodkray status`.
pub fn run(ctx: &Context, _args: StatusArgs) -> NodkrayResult<i32> {
    let report = StatusReport {
        tasks: active_tasks(),
    };

    if ctx.output.json {
        ctx.output.emit_json(&report);
    } else if report.tasks.is_empty() {
        ctx.output.emit_text("no active tasks");
    } else {
        for task in &report.tasks {
            ctx.output
                .emit_text(format!("{} [{}] {}", task.id, task.state, task.title));
        }
    }

    Ok(0)
}
