//! `nodkray config get|set` (spec §61, §126).
//!
//! `get` reads the effective (cascaded) value; `--global` restricts it to the
//! global layer. `set` writes to the project config when it exists, otherwise
//! to the global config; `--global` forces the global file.

use clap::{Args, Subcommand};
use serde_json::json;
use serde_yaml::Value;

use crate::cli::Context;
use crate::config::{get_path, load_effective_value, load_global_value, set_in_file};
use crate::error::{NodkrayError, NodkrayResult};

/// Arguments for `nodkray config`.
#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print a configuration value by dotted key.
    Get(GetArgs),
    /// Write a configuration value by dotted key.
    Set(SetArgs),
}

/// Arguments for `nodkray config get`.
#[derive(Debug, Args)]
pub struct GetArgs {
    /// Dotted key, e.g. `execution.backend`.
    pub key: String,
    /// Read from the global config layer only.
    #[arg(long)]
    pub global: bool,
}

/// Arguments for `nodkray config set`.
#[derive(Debug, Args)]
pub struct SetArgs {
    /// Dotted key, e.g. `review.default_depth`.
    pub key: String,
    /// New value (parsed as YAML scalar).
    pub value: String,
    /// Write to the global config file.
    #[arg(long)]
    pub global: bool,
}

/// Run `nodkray config`.
pub fn run(ctx: &Context, args: ConfigArgs) -> NodkrayResult<i32> {
    match args.command {
        ConfigCommand::Get(args) => get(ctx, args),
        ConfigCommand::Set(args) => set(ctx, args),
    }
}

fn get(ctx: &Context, args: GetArgs) -> NodkrayResult<i32> {
    let value = if args.global {
        load_global_value(&ctx.paths)?
    } else {
        let root = ctx.project_root().root;
        load_effective_value(&ctx.paths, Some(&root))?
    };

    let found = get_path(&value, &args.key).ok_or_else(|| {
        NodkrayError::user_input(
            "CONFIG_KEY_NOT_FOUND",
            format!("unknown configuration key `{}`", args.key),
        )
    })?;

    ctx.output
        .emit_json(&json!({ "key": &args.key, "value": to_json(found) }));
    ctx.output.emit_text(to_display(found));
    Ok(0)
}

fn set(ctx: &Context, args: SetArgs) -> NodkrayResult<i32> {
    let root = ctx.project_root().root;
    let project_file = ctx.paths.project_config_file(&root);

    let (path, scope) = if args.global || !project_file.exists() {
        (ctx.paths.global_config_file(), "global")
    } else {
        (project_file, "project")
    };

    let parsed = set_in_file(&path, &args.key, &args.value)?;
    tracing::info!(key = args.key, scope, "config value written");

    ctx.output.emit_json(&json!({
        "key": &args.key,
        "value": to_json(&parsed),
        "scope": scope,
        "path": path.display().to_string(),
    }));
    ctx.output.emit_text(format!(
        "Set {} = {} ({})",
        args.key,
        to_display(&parsed),
        path.display()
    ));
    Ok(0)
}

fn to_json(value: &Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn to_display(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_string(),
        other => serde_yaml::to_string(other)
            .unwrap_or_else(|_| format!("{other:?}"))
            .trim_end()
            .to_string(),
    }
}
