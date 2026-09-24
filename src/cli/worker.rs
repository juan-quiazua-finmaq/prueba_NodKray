//! `nodkray worker list|inspect` (spec §44).

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::memory::Worker;

#[derive(Debug, Args)]
pub struct WorkerArgs {
    #[command(subcommand)]
    pub command: WorkerCommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerCommand {
    List,
    Inspect { id: String },
}

#[derive(Debug, Serialize)]
struct WorkerView {
    #[serde(flatten)]
    worker: Worker,
    worktree_exists: bool,
}

pub fn run(ctx: &Context, args: WorkerArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;
    match args.command {
        WorkerCommand::List => {
            let workers = repo.list_workers(Some(&project.id))?;
            let views: Vec<WorkerView> = workers.into_iter().map(to_view).collect();
            ctx.output.emit_json(&views);
            if !ctx.output.json {
                for view in &views {
                    ctx.output.emit_text(format!(
                        "{}  {}  {}  {}",
                        view.worker.id,
                        view.worker.status,
                        view.worker.role,
                        view.worker.worktree_path.as_deref().unwrap_or("-")
                    ));
                }
            }
            Ok(0)
        }
        WorkerCommand::Inspect { id } => {
            let worker = repo.worker(&id)?.ok_or_else(|| {
                NodkrayError::user_input("WORKER_NOT_FOUND", format!("unknown worker {id}"))
            })?;
            let view = to_view(worker);
            ctx.output.emit_json(&view);
            if !ctx.output.json {
                ctx.output
                    .emit_text(serde_json::to_string_pretty(&view).unwrap_or_default());
            }
            Ok(0)
        }
    }
}

fn to_view(worker: Worker) -> WorkerView {
    let worktree_exists = worker
        .worktree_path
        .as_ref()
        .map(|path| std::path::Path::new(path).exists())
        .unwrap_or(false);
    WorkerView {
        worker,
        worktree_exists,
    }
}
