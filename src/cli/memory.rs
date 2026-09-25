//! `nodkray memory` (spec §46-§58).
//!
//! `search` is project-scoped by default; `--global` removes the project filter
//! (spec §160). Search never returns full content, only previews.

use clap::{Args, Subcommand};
use serde_json::json;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::memory::{MemoryRepository, NewMemory, MEMORY_TYPES};

/// Arguments for `nodkray memory`.
#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Store a memory entry.
    Save(SaveArgs),
    /// Full-text search (project-scoped unless `--global`).
    Search(SearchArgs),
    /// Print the full content of a memory or decision.
    Get(GetArgs),
    /// Recent history of the current project.
    Timeline(TimelineArgs),
    /// Repair the SQLite memory database (retry, WAL, backup + recreate).
    Repair(RepairArgs),
}

/// Arguments for `nodkray memory repair`.
#[derive(Debug, Args)]
pub struct RepairArgs {}

/// Arguments for `nodkray memory save`.
#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Memory type (decision|convention|bugfix|discovery|observation).
    #[arg(long = "type")]
    pub memory_type: String,
    /// Short title.
    #[arg(long)]
    pub title: String,
    /// Full content.
    #[arg(long)]
    pub content: String,
    /// Importance weight (default 0).
    #[arg(long, default_value_t = 0)]
    pub importance: i64,
    /// Optional source reference.
    #[arg(long)]
    pub source: Option<String>,
}

/// Arguments for `nodkray memory search`.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Free-text query.
    pub query: String,
    /// Maximum number of results.
    #[arg(long, default_value_t = 10)]
    pub limit: u32,
    /// Result offset for pagination.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
    /// Search across all projects (no project filter).
    #[arg(long)]
    pub global: bool,
}

/// Arguments for `nodkray memory get`.
#[derive(Debug, Args)]
pub struct GetArgs {
    /// Memory or decision id (`memory_<ulid>` / `decision_<ulid>`).
    pub id: String,
}

/// Arguments for `nodkray memory timeline`.
#[derive(Debug, Args)]
pub struct TimelineArgs {
    /// Maximum number of entries.
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
}

/// Run `nodkray memory`.
pub fn run(ctx: &Context, args: MemoryArgs) -> NodkrayResult<i32> {
    match args.command {
        MemoryCommand::Save(args) => save(ctx, args),
        MemoryCommand::Search(args) => search(ctx, args),
        MemoryCommand::Get(args) => get(ctx, args),
        MemoryCommand::Timeline(args) => timeline(ctx, args),
        MemoryCommand::Repair(_) => repair(ctx),
    }
}

fn repair(ctx: &Context) -> NodkrayResult<i32> {
    let root = ctx.project_root();
    let config = crate::config::load_effective(&ctx.paths, Some(&root.root))?;
    let db_path = ctx.paths.resolve_memory_path(&config.memory.path);
    let (_conn, report) = crate::memory::heal::heal(&db_path)?;
    ctx.output.emit_json(&report);
    ctx.output.emit_text(format!(
        "{} ({}){}",
        report.path,
        report.message,
        report
            .backup
            .as_ref()
            .map(|b| format!("; backup {b}"))
            .unwrap_or_default()
    ));
    Ok(0)
}

fn save(ctx: &Context, args: SaveArgs) -> NodkrayResult<i32> {
    if !MEMORY_TYPES.contains(&args.memory_type.as_str()) {
        return Err(NodkrayError::user_input(
            "MEMORY_TYPE_INVALID",
            format!(
                "unknown memory type `{}` (expected one of: {})",
                args.memory_type,
                MEMORY_TYPES.join(", ")
            ),
        ));
    }
    if args.title.trim().is_empty() {
        return Err(NodkrayError::user_input(
            "MEMORY_TITLE_REQUIRED",
            "a memory title is required",
        ));
    }

    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;
    let saved = repo.save_memory(&NewMemory {
        project_id: project.id,
        memory_type: args.memory_type,
        title: args.title,
        content: args.content,
        source: args.source,
        importance: args.importance,
    })?;

    tracing::info!(id = saved.id, "memory saved");
    ctx.output.emit_json(&json!({
        "id": saved.id,
        "type": saved.memory_type,
        "title": saved.title,
    }));
    ctx.output.emit_text(&saved.id);
    Ok(0)
}

fn search(ctx: &Context, args: SearchArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project_id = if args.global {
        None
    } else {
        Some(ctx.memory_project(&repo)?.id)
    };

    let results = repo.search_memory(
        &args.query,
        project_id.as_deref(),
        args.limit,
        args.offset,
    )?;

    if ctx.output.json {
        ctx.output.emit_json(&json!({
            "query": args.query,
            "global": args.global,
            "count": results.len(),
            "results": results,
        }));
    } else if results.is_empty() {
        ctx.output
            .emit_text(format!("no memories found for `{}`", args.query));
    } else {
        for result in &results {
            ctx.output.emit_text(format!(
                "{}  {:.3}  [{}]  {}\n    {}",
                result.id, result.score, result.memory_type, result.title, result.preview
            ));
        }
    }

    Ok(0)
}

fn get(ctx: &Context, args: GetArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    if let Some(memory) = repo.get_memory(&args.id)? {
        ctx.output.emit_json(&memory);
        ctx.output.emit_text(&memory.content);
        return Ok(0);
    }
    if let Some(decision) = repo.get_decision(&args.id)? {
        ctx.output.emit_json(&decision);
        let text = match &decision.rationale {
            Some(rationale) => format!("{}\n\n{}", decision.decision, rationale),
            None => decision.decision.clone(),
        };
        ctx.output.emit_text(&text);
        return Ok(0);
    }
    Err(NodkrayError::user_input(
        "MEMORY_NOT_FOUND",
        format!("unknown memory or decision id `{}`", args.id),
    ))
}

fn timeline(ctx: &Context, args: TimelineArgs) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;
    let entries = repo.timeline(Some(&project.id), args.limit)?;

    if ctx.output.json {
        ctx.output.emit_json(&json!({
            "project_id": project.id,
            "entries": entries,
        }));
    } else if entries.is_empty() {
        ctx.output.emit_text("no memory entries");
    } else {
        for entry in &entries {
            ctx.output.emit_text(format!(
                "{}  [{}]  {}  {}",
                entry.created_at, entry.kind, entry.id, entry.title
            ));
        }
    }

    Ok(0)
}
