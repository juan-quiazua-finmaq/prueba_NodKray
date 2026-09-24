//! `nodkray review` and `nodkray review inspect` (spec §44).

use clap::{Args, Subcommand};
use serde_json::json;

use crate::cli::Context;
use crate::core::workflow::ReviewDepth;
use crate::error::{NodkrayError, NodkrayResult};
use crate::review::engine::{run_review, ReviewRequest};

#[derive(Debug, Args)]
pub struct ReviewArgs {
    #[command(subcommand)]
    pub command: Option<ReviewCommand>,
    #[arg(long)]
    pub depth: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Show a persisted review (`review_<ulid>` or `task_<ulid>`).
    Inspect { id: String },
}

pub fn run(ctx: &Context, args: ReviewArgs) -> NodkrayResult<i32> {
    match args.command {
        Some(ReviewCommand::Inspect { id }) => inspect(ctx, &id),
        None => run_adhoc(ctx, args.depth.as_deref()),
    }
}

fn inspect(ctx: &Context, id: &str) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let review = if let Some(review) = repo.review(id)? {
        review
    } else if let Some(review) = repo.latest_review_for_task(id)? {
        review
    } else {
        return Err(NodkrayError::user_input(
            "REVIEW_NOT_FOUND",
            format!("no review found for `{id}`"),
        ));
    };
    let checks = repo.list_review_checks(&review.id)?;
    let payload = json!({ "review": review, "checks": checks });
    ctx.output.emit_json(&payload);
    if !ctx.output.json {
        ctx.output.emit_text(format!(
            "{}  [{}]  depth={}  task={}",
            review.id, review.status, review.depth, review.task_id
        ));
        for check in checks {
            ctx.output
                .emit_text(format!("  {}  {}", check.check_id, check.status));
        }
    }
    Ok(0)
}

fn run_adhoc(ctx: &Context, depth_flag: Option<&str>) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let depth = depth_flag
        .and_then(ReviewDepth::parse)
        .unwrap_or(ReviewDepth::Balanced);
    let repo = ctx.open_memory()?;
    let verdict = run_review(
        &repo,
        &ReviewRequest {
            task_id: "review_adhoc".to_string(),
            depth: depth.as_str().to_string(),
            root: project.root.clone(),
            worktree: None,
            base_branch: "HEAD".to_string(),
            policy: project.config.review.policy.clone(),
            test_timeout: std::time::Duration::from_secs(120),
            sentrux_enabled: project.config.integrations.sentrux.enabled,
            sentrux_required: project.config.integrations.sentrux.required,
            reviewer_available: false,
        },
    )?;
    ctx.output.emit_json(&verdict);
    if !ctx.output.json {
        ctx.output
            .emit_text(serde_json::to_string_pretty(&verdict).unwrap_or_default());
    }
    match verdict.status.as_str() {
        "passed" => Ok(0),
        "blocked" => Err(NodkrayError::internal("BLOCKED", "RDD verdict is blocked")),
        _ => Err(NodkrayError::review("REVIEW_FAILED", "RDD verdict is failed")),
    }
}
