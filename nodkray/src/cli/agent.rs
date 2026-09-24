//! `nodkray agent` — scaffolded in phase 0, implemented in phase 4 (spec §7-§9).

use clap::Args;

use crate::cli::{not_implemented, Context};
use crate::error::NodkrayResult;

/// Arguments for `nodkray agent` (parsed but not yet acted upon).
#[derive(Debug, Args)]
pub struct AgentArgs {
    /// Forwarded arguments (`list`, `inspect`, ...).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Run `nodkray agent` (phase 0 stub).
pub fn run(_ctx: &Context, _args: AgentArgs) -> NodkrayResult<i32> {
    Err(not_implemented("agent"))
}
