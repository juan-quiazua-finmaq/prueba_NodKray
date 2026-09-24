//! `nodkray uninstall`.
//!
//! Removes NodKray's own files only. MCP servers, Spec-Kit, Herdr and any
//! tool configs that were already on the machine stay put.

use std::io::Write;

use clap::Args;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::lifecycle;
use crate::installer::prompt::{is_interactive, prompt_yes_no};

#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Also remove NodKray overlay files from the current project
    /// (`.nodkray/`, `skills/nodkray/`, the AGENTS.md / gitignore blocks).
    /// Never deletes `.serena/`, `.codegraph/`, `.sentrux/`, `.specify/` or `.herdr/`.
    #[arg(long)]
    pub project: bool,
    /// Do not ask for confirmation.
    #[arg(long)]
    pub yes: bool,
}

pub fn run(ctx: &Context, args: UninstallArgs) -> NodkrayResult<i32> {
    if is_interactive() && !args.yes {
        let question = if args.project {
            "Remove NodKray (binary, config, memory) and project overlay files? MCP/tool configs will be kept."
        } else {
            "Remove NodKray binary, config and memory? MCP/tool configs will be kept."
        };
        if !prompt_yes_no(question, false)? {
            ctx.output.emit_text("uninstall cancelled");
            return Ok(0);
        }
    } else if !args.yes && !ctx.output.json {
        let _ = writeln!(
            std::io::stderr(),
            "re-run with --yes to uninstall without a prompt"
        );
        return Err(NodkrayError::user_input(
            "UNINSTALL_CONFIRM",
            "pass --yes to uninstall without a prompt",
        ));
    }

    let report = lifecycle::uninstall(&ctx.paths, &ctx.cwd, args.project)?;
    ctx.output.emit_json(&report);
    ctx.output.emit_text("NodKray uninstall");
    for path in &report.removed {
        ctx.output.emit_text(format!("  removed {path}"));
    }
    for path in &report.skipped {
        ctx.output.emit_text(format!("  skipped {path}"));
    }
    ctx.output.emit_text(&report.note);
    Ok(0)
}
