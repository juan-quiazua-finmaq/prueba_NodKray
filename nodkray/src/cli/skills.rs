//! Explicit skills / AGENTS.md install (phase 5). Not part of `init`.

use clap::Args;

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::installer::{agents_md, skills};

#[derive(Debug, Args)]
pub struct SkillsArgs {
    /// Also insert the delimited NodKray block into AGENTS.md.
    #[arg(long)]
    pub agents_md: bool,
}

pub fn run(ctx: &Context, args: SkillsArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let written = skills::install_into(&project.root)?;
    ctx.output.emit_text(format!(
        "skills installed: {}",
        if written.is_empty() {
            "already present".to_string()
        } else {
            written.join(", ")
        }
    ));
    if args.agents_md {
        let changed = agents_md::ensure_block(&project.root)?;
        ctx.output.emit_text(if changed {
            "AGENTS.md: NodKray block written"
        } else {
            "AGENTS.md: NodKray block already present"
        });
    }
    ctx.output.emit_json(&serde_json::json!({
        "skills": written,
        "agents_md": args.agents_md,
    }));
    Ok(0)
}
