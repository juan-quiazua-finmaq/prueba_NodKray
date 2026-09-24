//! `nodkray agent list|inspect` (spec §8, §44).

use clap::{Args, Subcommand};

use crate::agents;
use crate::cli::Context;
use crate::error::NodkrayResult;

#[derive(Debug, Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// List known adapters and detection status.
    List,
    /// Inspect one adapter's capabilities.
    Inspect {
        id: String,
    },
}

pub fn run(ctx: &Context, args: AgentArgs) -> NodkrayResult<i32> {
    match args.command {
        AgentCommand::List => {
            let agents = agents::list();
            ctx.output.emit_json(&agents);
            if !ctx.output.json {
                for agent in agents {
                    let mark = if agent.detection.installed { "ok" } else { "--" };
                    ctx.output.emit_text(format!(
                        "[{mark}] {} ({})",
                        agent.id, agent.executable
                    ));
                }
            }
            Ok(0)
        }
        AgentCommand::Inspect { id } => {
            let info = agents::inspect(&id)?;
            ctx.output.emit_json(&info);
            ctx.output.emit_text(serde_json::to_string_pretty(&info).unwrap_or_default());
            Ok(0)
        }
    }
}
