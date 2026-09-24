//! `nodkray memory` — scaffolded in phase 0, implemented in phase 1 (spec §58).

use clap::Args;

use crate::cli::{not_implemented, Context};
use crate::error::NodkrayResult;

/// Arguments for `nodkray memory` (parsed but not yet acted upon).
#[derive(Debug, Args)]
pub struct MemoryArgs {
    /// Forwarded arguments (`save`, `search`, `get`, `timeline`, ...).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Run `nodkray memory` (phase 0 stub).
pub fn run(_ctx: &Context, _args: MemoryArgs) -> NodkrayResult<i32> {
    Err(not_implemented("memory"))
}
