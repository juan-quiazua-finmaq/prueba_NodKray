//! `nodkray mcp list|status` (spec §44, §96).

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::mcp::{self, McpServer};

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

#[derive(Debug, Serialize)]
struct McpStatus {
    servers: Vec<McpServer>,
    available: usize,
    configured: usize,
    missing: Vec<String>,
}

pub fn run(ctx: &Context, args: McpArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let servers = mcp::discover(Some(&project.root), &project.config);
    match args.command {
        McpCommand::List => {
            ctx.output.emit_json(&servers);
            if !ctx.output.json {
                for server in &servers {
                    emit_server(ctx, server);
                }
            }
            Ok(0)
        }
        McpCommand::Status => {
            let available = servers.iter().filter(|server| server.available).count();
            let configured = servers.iter().filter(|server| server.configured).count();
            let missing: Vec<String> = servers
                .iter()
                .filter(|server| server.configured && !server.installed)
                .map(|server| server.name.clone())
                .collect();
            let status = McpStatus {
                servers: servers.clone(),
                available,
                configured,
                missing,
            };
            ctx.output.emit_json(&status);
            if !ctx.output.json {
                ctx.output.emit_text(format!(
                    "mcp available={available} configured={configured}"
                ));
                for server in &status.servers {
                    emit_server(ctx, server);
                }
            }
            Ok(0)
        }
    }
}

fn emit_server(ctx: &Context, server: &McpServer) {
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
