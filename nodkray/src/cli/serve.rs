//! `nodkray serve` — optional Control API (spec §86-§87).

use std::sync::Arc;

use clap::Args;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::remote::{serve, CoreControlService, ServeOptions};

/// Arguments for `nodkray serve`.
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Override `remote.bind` for this process only.
    #[arg(long)]
    pub bind: Option<String>,
}

/// Run the control API in the foreground. Explicit opt-in for this process;
/// does not persist `remote.enabled` to disk.
pub fn run(ctx: &Context, args: ServeArgs) -> NodkrayResult<i32> {
    let project = ctx.project_context()?;
    let mut remote = project.config.remote.clone();
    if let Some(bind) = args.bind {
        remote.bind = bind;
    }

    let token = std::env::var(remote.token_env_name())
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            NodkrayError::permission(
                "AUTH_TOKEN_MISSING",
                format!(
                    "set {} before starting the control API",
                    remote.token_env_name()
                ),
            )
        })?;

    ctx.output.emit_text(format!(
        "control API listening on {} (loopback, authenticated)",
        remote.bind
    ));
    ctx.output.emit_json(&serde_json::json!({
        "bind": remote.bind,
        "authenticated": true,
    }));

    serve(
        ServeOptions {
            bind: remote.bind,
            token,
        },
        Arc::new(CoreControlService::new(ctx.paths.clone(), ctx.cwd.clone())),
    )?;
    Ok(0)
}
