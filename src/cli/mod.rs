//! Command-line entry point.
//!
//! Handlers only parse arguments and delegate: all business logic lives in
//! `installer`, `config` and `core`. Global flags (`--json`, `--quiet`,
//! `--verbose`) are available on every subcommand.

pub mod agent;
pub mod config;
pub mod doctor;
pub mod init;
pub mod mcp;
pub mod memory;
pub mod project;
pub mod review;
pub mod serve;
pub mod skills;
pub mod status;
pub mod task;
pub mod uninstall;
pub mod update;
pub mod worker;

use std::fmt::Display;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::config::{load_effective, ConfigPaths};
use crate::core::project::{find_project_root, ProjectContext, ProjectRoot};
use crate::error::{NodkrayError, NodkrayResult};
use crate::logging;
use crate::memory::sqlite::SqliteMemoryRepository;
use crate::memory::Project;

/// NodKray — local-first agent orchestration CLI.
#[derive(Debug, Parser)]
#[command(name = "nodkray", version, about, long_about = None)]
pub struct Cli {
    /// Emit machine-readable JSON on stdout (logs stay on stderr).
    #[arg(long, global = true)]
    pub json: bool,
    /// Suppress normal stdout output (`--json` still works).
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Enable verbose logging on stderr.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialise NodKray globally and/or in the current project.
    Init(init::InitArgs),
    /// Diagnose the environment and configuration.
    Doctor(doctor::DoctorArgs),
    /// Show active tasks (or `--all` for recent terminal ones).
    Status(status::StatusArgs),
    /// Read or write configuration values.
    Config(config::ConfigArgs),
    /// Create, run, inspect and cancel tasks (ST = small, ODD = task.md, SDD = Spec-Kit).
    Task(task::TaskArgs),
    /// Persistent memory (SQLite + FTS5).
    Memory(memory::MemoryArgs),
    /// Review workflows (RDD).
    Review(review::ReviewArgs),
    /// Inspect agents.
    Agent(agent::AgentArgs),
    /// Inspect the current project (identity, rules, counts).
    Project(project::ProjectArgs),
    /// Inspect workers and worktrees.
    Worker(worker::WorkerArgs),
    /// Discover configured MCP servers.
    Mcp(mcp::McpArgs),
    /// Install NodKray skills (and optionally the AGENTS.md block).
    Skills(skills::SkillsArgs),
    /// Start the optional loopback Control API.
    Serve(serve::ServeArgs),
    /// Download the latest (or pinned) release binary.
    Update(update::UpdateArgs),
    /// Remove NodKray files. Never deletes pre-existing MCP or tool configs.
    Uninstall(uninstall::UninstallArgs),
}

/// Shared execution context handed to every handler.
#[derive(Debug)]
pub struct Context {
    pub paths: ConfigPaths,
    pub output: Output,
    pub cwd: PathBuf,
}

impl Context {
    /// Resolve the project root for the current directory.
    pub fn project_root(&self) -> ProjectRoot {
        find_project_root(&self.cwd)
    }

    /// Discover the project and load its effective configuration.
    pub fn project_context(&self) -> NodkrayResult<ProjectContext> {
        ProjectContext::discover(&self.cwd, &self.paths)
    }

    /// Open the SQLite memory database for the current configuration.
    pub fn open_memory(&self) -> NodkrayResult<SqliteMemoryRepository> {
        let root = self.project_root();
        let config = load_effective(&self.paths, Some(&root.root))?;
        let db_path = self.paths.resolve_memory_path(&config.memory.path);
        SqliteMemoryRepository::open(&db_path)
    }

    /// Resolve (create if needed) the persistent project for the current cwd.
    pub fn memory_project(&self, repo: &SqliteMemoryRepository) -> NodkrayResult<Project> {
        let root = self.project_root();
        if root.git_root.is_none() {
            tracing::warn!(root = %root.root.display(), "no git repository found; using cwd as project root");
            self.output
                .warn(&format!("no git repository found; using {} as project root", root.root.display()));
        }
        crate::core::project::resolve_project(repo, &root.root)
    }
}

/// stdout rendering policy.
#[derive(Debug, Clone, Copy)]
pub struct Output {
    pub json: bool,
    pub quiet: bool,
}

impl Output {
    /// Human-readable message; suppressed by `--json` and `--quiet`.
    pub fn emit_text(&self, message: impl Display) {
        if !self.json && !self.quiet {
            println!("{message}");
        }
    }

    /// Always print a human-readable message unless `--json` (used by `doctor`).
    pub fn emit_text_unquiet(&self, message: impl Display) {
        if !self.json {
            println!("{message}");
        }
    }

    /// Emit a warning on stderr (never stdout, so `--json` stays valid).
    pub fn warn(&self, message: impl Display) {
        eprintln!("warning: {message}");
    }

    /// Print `value` as a single JSON document when `--json` is active.
    pub fn emit_json<T: Serialize>(&self, value: &T) {
        if self.json {
            if let Ok(text) = serde_json::to_string(value) {
                println!("{text}");
            }
        }
    }

    /// Render a terminal error: JSON on stdout for `--json`, else stderr.
    pub fn emit_error(&self, error: &NodkrayError) {
        if self.json {
            if let Ok(text) = serde_json::to_string(&error.to_json()) {
                println!("{text}");
            }
        } else {
            eprintln!("error: {error}");
        }
    }
}

/// Parse arguments, configure logging and dispatch. Returns the exit code.
pub fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // clap handles `--help`/`--version` (exit 0) and usage errors (exit 2).
        Err(err) => err.exit(),
    };

    let paths = ConfigPaths::from_env();
    // Keep the log guard alive for the whole process.
    let _log_guard = logging::init(&paths, cli.verbose);

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let ctx = Context {
        paths,
        output: Output {
            json: cli.json,
            quiet: cli.quiet,
        },
        cwd,
    };

    tracing::debug!(cwd = %ctx.cwd.display(), "nodkray starting");

    let result = match cli.command {
        Command::Init(args) => init::run(&ctx, args),
        Command::Doctor(args) => doctor::run(&ctx, args),
        Command::Status(args) => status::run(&ctx, args),
        Command::Config(args) => config::run(&ctx, args),
        Command::Task(args) => task::run(&ctx, args),
        Command::Memory(args) => memory::run(&ctx, args),
        Command::Review(args) => review::run(&ctx, args),
        Command::Agent(args) => agent::run(&ctx, args),
        Command::Project(args) => project::run(&ctx, args),
        Command::Worker(args) => worker::run(&ctx, args),
        Command::Mcp(args) => mcp::run(&ctx, args),
        Command::Skills(args) => skills::run(&ctx, args),
        Command::Serve(args) => serve::run(&ctx, args),
        Command::Update(args) => update::run(&ctx, args),
        Command::Uninstall(args) => uninstall::run(&ctx, args),
    };

    match result {
        Ok(exit_code) => exit_code,
        Err(error) => {
            tracing::error!(code = error.code(), message = error.message(), "command failed");
            ctx.output.emit_error(&error);
            error.exit_code()
        }
    }
}

/// Helper for phase-0 stubs.
pub fn not_implemented(command: &str) -> NodkrayError {
    NodkrayError::internal(
        "NOT_IMPLEMENTED",
        format!("`{command}` is not implemented in phase 0"),
    )
}
