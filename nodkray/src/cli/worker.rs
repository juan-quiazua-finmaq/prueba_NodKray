//! `nodkray worker list|inspect` (spec §44).

use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::execution::worktree_path;

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
struct WorkerRecord {
    id: String,
    worktree: String,
    exists: bool,
}

pub fn run(ctx: &Context, args: WorkerArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    match args.command {
        WorkerCommand::List => {
            let workers = list_workers(&project.root);
            ctx.output.emit_json(&workers);
            if !ctx.output.json {
                for worker in workers {
                    ctx.output.emit_text(format!(
                        "{}  {}  {}",
                        worker.id,
                        if worker.exists { "ready" } else { "missing" },
                        worker.worktree
                    ));
                }
            }
            Ok(0)
        }
        WorkerCommand::Inspect { id } => {
            let path = worktree_path(&project.root, &id);
            if !path.exists() {
                return Err(NodkrayError::user_input(
                    "WORKER_NOT_FOUND",
                    format!("no worktree for worker {id}"),
                ));
            }
            let record = WorkerRecord {
                id,
                exists: true,
                worktree: path.display().to_string(),
            };
            ctx.output.emit_json(&record);
            ctx.output.emit_text(serde_json::to_string_pretty(&record).unwrap_or_default());
            Ok(0)
        }
    }
}

fn list_workers(root: &Path) -> Vec<WorkerRecord> {
    let dir = root.join(".nodkray").join("worktrees");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            let id = entry.file_name().to_string_lossy().into_owned();
            out.push(WorkerRecord {
                exists: true,
                worktree: entry.path().display().to_string(),
                id,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
