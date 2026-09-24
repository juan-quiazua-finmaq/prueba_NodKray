//! `nodkray init` (spec §64-§65, §98).

use std::io::IsTerminal;
use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::cli::Context;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::project::{init_global, init_project, GlobalInitReport, ProjectInitReport};
use crate::installer::{environment_report, EnvironmentReport};

/// Arguments for `nodkray init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Initialise the global configuration only.
    #[arg(long, conflicts_with = "project")]
    pub global: bool,
    /// Initialise the project configuration only.
    #[arg(long, conflicts_with = "global")]
    pub project: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Global,
    Project,
    Both,
}

impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project => "project",
            Scope::Both => "both",
        }
    }
}

impl InitArgs {
    /// Resolve the requested scope. Without flags, default to `Both`: the
    /// global config is always ensured and the project config is created when a
    /// git repository is present. On a TTY the user is asked first.
    fn scope(&self, cwd: &Path) -> NodkrayResult<Scope> {
        if self.global {
            return Ok(Scope::Global);
        }
        if self.project {
            return Ok(Scope::Project);
        }
        let default = Scope::Both;
        let _ = cwd;

        if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            eprint!("NodKray init scope [g]lobal/[p]roject/[b]oth (default b): ");
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|err| NodkrayError::user_input("INIT_INPUT_ERROR", err.to_string()))?;
            return Ok(match line.trim().to_ascii_lowercase().as_str() {
                "g" | "global" => Scope::Global,
                "p" | "project" => Scope::Project,
                "b" | "both" => Scope::Both,
                _ => default,
            });
        }

        Ok(default)
    }
}

/// JSON/text report for `init`.
#[derive(Debug, Serialize)]
struct InitReport {
    scope: String,
    global: Option<GlobalInitReport>,
    project: Option<ProjectInitReport>,
    environment: EnvironmentReport,
}

/// Run `nodkray init`.
pub fn run(ctx: &Context, args: InitArgs) -> NodkrayResult<i32> {
    let scope = args.scope(&ctx.cwd)?;
    let root = ctx.project_root();

    let run_global = matches!(scope, Scope::Global | Scope::Both);
    let run_project = match scope {
        Scope::Project => true,
        // Default scope only initialises the project when inside a git repo.
        Scope::Both => root.git_root.is_some(),
        Scope::Global => false,
    };

    let global = if run_global {
        Some(init_global(&ctx.paths)?)
    } else {
        None
    };
    let project = if run_project {
        Some(init_project(&ctx.paths, &ctx.cwd)?)
    } else {
        None
    };

    let env_root = project
        .as_ref()
        .map(|p| Path::new(p.root.as_str()).to_path_buf())
        .unwrap_or_else(|| root.root.clone());
    let environment = environment_report(Some(&env_root));

    let report = InitReport {
        scope: scope.as_str().to_string(),
        global,
        project,
        environment,
    };

    tracing::info!(scope = report.scope, "init complete");
    ctx.output.emit_json(&report);
    ctx.output.emit_text(render_text(&report));
    Ok(0)
}

fn render_text(report: &InitReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("NodKray init (scope: {})\n\n", report.scope));

    match &report.global {
        Some(global) => {
            let state = if global.created { "created" } else { "exists" };
            out.push_str(&format!("Global config: {} ({})\n", global.path, state));
        }
        None => out.push_str("Global config: skipped\n"),
    }

    match &report.project {
        Some(project) => {
            out.push_str(&format!("Project root: {}\n", project.root));
            out.push_str(&format!(
                "Project config: {} ({})\n",
                project.config_path,
                if project.created.iter().any(|c| c == ".nodkray/config.yaml") {
                    "created"
                } else {
                    "exists"
                }
            ));
        }
        None => out.push_str("Project config: skipped\n"),
    }

    let env = &report.environment;
    out.push_str(&format!("\nEnvironment\n"));
    out.push_str(&format!("  OS: {} ({})\n", env.os, env.arch));
    out.push_str(&format!(
        "  Git: {}\n",
        if env.git { "present" } else { "missing" }
    ));

    out.push_str("  Agents:\n");
    for agent in &env.agents {
        out.push_str(&format!(
            "    {} {}\n",
            if agent.found { "[OK]" } else { "[--]" },
            agent.executable
        ));
    }

    out.push_str("  Tools:\n");
    for tool in &env.tools {
        out.push_str(&format!(
            "    {} {}\n",
            if tool.found { "[OK]" } else { "[--]" },
            tool.name
        ));
    }

    if let Some(project) = &report.project {
        out.push_str("  Rules:\n");
        if project.rules.is_empty() {
            out.push_str("    [--] none detected\n");
        } else {
            for rule in &project.rules {
                out.push_str(&format!("    [OK] {}\n", rule.name));
            }
        }
    }

    out
}
