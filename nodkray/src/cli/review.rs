//! `nodkray review` and `review inspect` (spec §44).

use clap::{Args, Subcommand};

use crate::cli::Context;
use crate::core::workflow::ReviewDepth;
use crate::error::{NodkrayError, NodkrayResult};
use crate::review::{review, ReviewRequest};

#[derive(Debug, Args)]
pub struct ReviewArgs {
    #[command(subcommand)]
    pub command: Option<ReviewCommand>,
    /// Override review depth.
    #[arg(long)]
    pub depth: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Show the last/current review verdict for this workspace.
    Inspect,
}

pub fn run(ctx: &Context, args: ReviewArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let depth = args
        .depth
        .as_deref()
        .and_then(ReviewDepth::parse)
        .unwrap_or(ReviewDepth::Balanced);

    let verdict = review(ReviewRequest {
        root: &project.root,
        depth,
        config: &project.config,
        reviewer_available: crate::agents::inspect(&project.config.agents.workers.reviewer.provider)
            .map(|a| a.detection.installed)
            .unwrap_or(false),
    });

    ctx.output.emit_json(&verdict);
    if !ctx.output.json {
        ctx.output
            .emit_text(serde_json::to_string_pretty(&verdict).unwrap_or_default());
    }

    match verdict.status {
        crate::review::VerdictStatus::Passed => Ok(0),
        crate::review::VerdictStatus::Failed => Err(NodkrayError::review(
            "REVIEW_FAILED",
            "RDD verdict is failed",
        )),
        crate::review::VerdictStatus::Blocked => Err(NodkrayError::internal(
            "BLOCKED",
            "RDD verdict is blocked",
        )),
    }
}
