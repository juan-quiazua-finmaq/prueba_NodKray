//! `nodkray doctor` (spec §70).

use clap::Args;

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::installer::{doctor, DoctorReport};

/// Arguments for `nodkray doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {}

/// Run `nodkray doctor`. Always returns a valid report; the exit code reflects
/// required failures only (missing Git, corrupt configuration).
pub fn run(ctx: &Context, _args: DoctorArgs) -> NodkrayResult<i32> {
    let report: DoctorReport = doctor(&ctx.paths, &ctx.cwd);
    tracing::info!(ok = report.ok, exit_code = report.exit_code, "doctor complete");

    if ctx.output.json {
        ctx.output.emit_json(&report);
    } else {
        ctx.output.emit_text_unquiet(report.render_text().trim_end());
    }

    Ok(report.exit_code)
}
