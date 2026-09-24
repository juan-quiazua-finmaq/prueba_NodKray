//! `nodkray update`.

use clap::Args;

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::installer::lifecycle;

#[derive(Debug, Args)]
pub struct UpdateArgs {}

pub fn run(ctx: &Context, _args: UpdateArgs) -> NodkrayResult<i32> {
    let report = lifecycle::update_binary(&ctx.paths, &ctx.cwd)?;
    ctx.output.emit_json(&report);
    ctx.output.emit_text(format!(
        "Updated {} from {} ({})",
        report.installed, report.repository, report.version
    ));
    if let Some(memory) = &report.memory {
        ctx.output
            .emit_text(format!("Memory: {} ({})", memory.path, memory.message));
    }
    Ok(0)
}
