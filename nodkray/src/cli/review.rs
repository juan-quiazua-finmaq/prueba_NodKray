//! `nodkray review` (spec §44).

use clap::Args;

use crate::cli::Context;
use crate::core::workflow::ReviewDepth;
use crate::error::{NodkrayError, NodkrayResult};
use crate::review::engine::{run_fast, ReviewRequest};

#[derive(Debug, Args)]
pub struct ReviewArgs {
    #[arg(long)]
    pub depth: Option<String>,
}

pub fn run(ctx: &Context, args: ReviewArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let depth = args
        .depth
        .as_deref()
        .and_then(ReviewDepth::parse)
        .unwrap_or(ReviewDepth::Balanced);
    let repo = ctx.open_memory()?;
    let verdict = run_fast(
        &repo,
        &ReviewRequest {
            task_id: "review_adhoc".to_string(),
            depth: depth.as_str().to_string(),
            root: project.root.clone(),
            worktree: None,
            base_branch: "HEAD".to_string(),
            policy: project.config.review.policy.clone(),
            test_timeout: std::time::Duration::from_secs(120),
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
