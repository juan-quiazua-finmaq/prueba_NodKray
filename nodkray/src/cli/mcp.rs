//! `nodkray mcp list|status` (spec §44, §96).

use clap::{Args, Subcommand};

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::mcp;

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    List,
    Status,
}

pub fn run(ctx: &Context, args: McpArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let servers = mcp::discover(Some(&project.root), &project.config);
    ctx.output.emit_json(&servers);
    if !ctx.output.json {
        for server in servers {
            let mark = if server.available {
                "ok"
            } else if server.installed {
                "cfg"
            } else {
                "--"
            };
            ctx.output.emit_text(format!(
                "[{mark}] {} installed={} configured={}",
                server.name, server.installed, server.configured
            ));
        }
    }
    let _ = args;
    Ok(0)
}
