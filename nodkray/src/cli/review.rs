//! `nodkray review` — scaffolded in phase 0, implemented in phases 2-5.

use clap::Args;

use crate::cli::{not_implemented, Context};
use crate::error::NodkrayResult;

/// Arguments for `nodkray review` (parsed but not yet acted upon).
#[derive(Debug, Args)]
pub struct ReviewArgs {
    /// Forwarded arguments (`inspect`, `--review fast|balanced|deep`, ...).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Run `nodkray review` (phase 0 stub).
pub fn run(_ctx: &Context, _args: ReviewArgs) -> NodkrayResult<i32> {
    Err(not_implemented("review"))
}
