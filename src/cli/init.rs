//! `nodkray init` (spec §64-§65, §98).

use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::cli::Context;
use crate::config::load_effective;
use crate::error::{NodkrayError, NodkrayResult};
use crate::installer::project::{init_global, init_project, GlobalInitReport, ProjectInitReport};
use crate::installer::prompt::{is_interactive, prompt_yes_no};
use crate::installer::provision::{self, ProvisionReport};
use crate::installer::wizard::{self, AgentChoices};
use crate::installer::{environment_report, owned, EnvironmentReport};

/// Arguments for `nodkray init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Initialise the global configuration only.
    #[arg(long, conflicts_with = "project")]
    pub global: bool,
    /// Initialise the project configuration only.
    #[arg(long, conflicts_with = "global")]
    pub project: bool,
    /// Accept defaults; no prompts. Does not install optional tools unless `--provision`.
    #[arg(long)]
    pub yes: bool,
    /// Install missing MCP servers and Spec-Kit (and Herdr if confirmed / `--yes`).
    #[arg(long)]
    pub provision: bool,
    /// Ask (or rewrite) agent-role assignments even when the config already exists.
    #[arg(long)]
    pub reconfigure: bool,
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
    fn scope(&self, cwd: &Path) -> NodkrayResult<Scope> {
        if self.global {
            return Ok(Scope::Global);
        }
        if self.project {
            return Ok(Scope::Project);
        }
        let default = Scope::Both;
        let _ = cwd;

        if is_interactive() && !self.yes {
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    agents_changed: Vec<String>,
    provision: ProvisionReport,
}

/// Run `nodkray init`.
pub fn run(ctx: &Context, args: InitArgs) -> NodkrayResult<i32> {
    let scope = args.scope(&ctx.cwd)?;
    let root = ctx.project_root();

    let run_global = matches!(scope, Scope::Global | Scope::Both);
    let run_project = match scope {
        Scope::Project => true,
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

    let config_target = if run_project {
        ctx.paths.project_config_file(&env_root)
    } else {
        ctx.paths.global_config_file()
    };
    let existing = load_effective(&ctx.paths, Some(&env_root)).ok();
    let choices = wizard::collect_choices(is_interactive() && !args.yes, args.yes)?;
    let mut agents_changed = wizard::apply_choices(
        &config_target,
        existing.as_ref(),
        &choices,
        args.reconfigure,
    )?;
    if !crate::installer::probes::herdr_runner_usable() {
        let current_backend = existing
            .as_ref()
            .map(|c| c.execution.backend.as_str())
            .unwrap_or("herdr");
        if current_backend != "console" {
            crate::config::set_in_file(&config_target, "execution.backend", "console")?;
            agents_changed.push("execution.backend".to_string());
        }
    }
    persist_update_repo(&config_target)?;

    let provision = run_provision(&args, &env_root, &ctx.paths.project_dir(&env_root))?;
    let environment = environment_report(Some(&env_root));

    let report = InitReport {
        scope: scope.as_str().to_string(),
        global,
        project,
        environment,
        agents_changed,
        provision,
    };

    tracing::info!(scope = report.scope, "init complete");
    ctx.output.emit_json(&report);
    ctx.output.emit_text(render_text(&report, &choices));
    Ok(0)
}

fn persist_update_repo(config_path: &Path) -> NodkrayResult<()> {
    let repo = crate::installer::lifecycle::resolve_repository(None);
    if repo.is_empty() {
        return Ok(());
    }
    crate::config::set_in_file(config_path, "update.repository", &repo)?;
    Ok(())
}

fn run_provision(
    args: &InitArgs,
    project_root: &Path,
    project_dir: &Path,
) -> NodkrayResult<ProvisionReport> {
    let interactive = is_interactive() && !args.yes;
    let mut report = ProvisionReport::default();

    let install_mcp = if args.provision {
        true
    } else if interactive {
        prompt_yes_no("Install missing MCP servers (Serena, CodeGraph, Sentrux) if not found?", true)?
    } else {
        false
    };
    if install_mcp {
        provision::provision_mcps(&mut report, true);
    } else {
        provision::provision_mcps(&mut report, false);
    }

    let specify_missing = crate::spec::speckit::SpecKitAdapter::detect(Some(project_root))
        .executable
        .is_none();
    if specify_missing {
        if interactive {
            eprintln!(
                "warning: Spec-Kit (`specify`) is not installed. SDD will fail until it is."
            );
        }
        let install_spec = args.provision
            || (interactive
                && prompt_yes_no("Install the latest Spec-Kit from github/spec-kit?", true)?);
        provision::provision_speckit(&mut report, install_spec);
    } else {
        provision::provision_speckit(&mut report, false);
    }

    let install_herdr = if crate::installer::tools::find_in_path("herdr").is_some() {
        false
    } else if args.provision && args.yes {
        true
    } else if interactive {
        prompt_yes_no("Install Herdr as the execution backend?", false)?
    } else {
        false
    };
    provision::provision_herdr(&mut report, install_herdr);

    if let Ok(Some(mut manifest)) = owned::OwnedManifest::load(project_dir) {
        manifest.provisioned = provision::newly_installed(&report);
        let _ = manifest.save(project_dir);
    } else if project_dir.is_dir() {
        let mut manifest = owned::OwnedManifest::detect(project_root);
        manifest.provisioned = provision::newly_installed(&report);
        let _ = manifest.save(project_dir);
    }

    Ok(report)
}

fn render_text(report: &InitReport, choices: &AgentChoices) -> String {
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
            if !project.skills.is_empty() {
                out.push_str(&format!("Skills: {}\n", project.skills.join(", ")));
            }
            if project.agents_md {
                out.push_str("AGENTS.md: NodKray block written\n");
            }
            if project.gitignore {
                out.push_str(".gitignore: NodKray block written\n");
            }
            if let Some(memory) = &project.memory {
                out.push_str(&format!("Memory: {} ({})\n", memory.path, memory.message));
            }
        }
        None => out.push_str("Project config: skipped\n"),
    }

    out.push_str(&format!(
        "\nAgents: frontier={} default={} backend={} frontend={} reviewer={} docs={}\n",
        choices.frontier,
        choices.default,
        choices.backend,
        choices.frontend,
        choices.reviewer,
        choices.docs
    ));
    if !report.agents_changed.is_empty() {
        out.push_str(&format!(
            "Updated keys: {}\n",
            report.agents_changed.join(", ")
        ));
    }

    if !report.provision.actions.is_empty() {
        out.push_str("\nProvisioning\n");
        for action in &report.provision.actions {
            let mark = if action.installed {
                "[OK]"
            } else if action.skipped {
                "[--]"
            } else {
                "[X]"
            };
            out.push_str(&format!("  {mark} {}: {}\n", action.name, action.detail));
        }
        out.push_str("Uninstall will not remove these tools if they already existed.\n");
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

    let herdr_usable = crate::installer::probes::herdr_runner_usable();
    out.push_str(&format!(
        "\nExecution backend: {}\n",
        if herdr_usable {
            "herdr (`herdr run` contract ok)"
        } else {
            "console (Herdr is present but has no `run` contract, or is missing)"
        }
    ));

    let constitution = crate::spec::constitution::CONSTITUTION_RELATIVE;
    out.push_str(&format!(
        "SDD constitution: {constitution} (required; init creates a stub if missing)\n"
    ));
    if crate::installer::tools::find_in_path("specify").is_some() {
        let integration = first_detected_integration(&choices);
        out.push_str(&format!(
            "Spec-Kit CLI: `specify init --here --force --ignore-agent-tools --integration {integration}`\n"
        ));
    } else {
        out.push_str("Spec-Kit CLI: not on PATH; SDD will fail with SPECKIT_NOT_FOUND\n");
    }

    out.push_str("\nNext steps\n");
    out.push_str("  1. nodkray doctor          # contract probes, not just PATH\n");
    out.push_str("  2. nodkray task --workflow st \"<small local edit>\"\n");
    out.push_str("  ST = small change · ODD = writes task.md · SDD = Spec-Kit /speckit.*\n");
    if let Some(project) = &report.project {
        out.push_str(&format!(
            "  Test skill: {}/skills/nodkray/test.md\n",
            project.root
        ));
    }

    out
}

fn first_detected_integration(choices: &AgentChoices) -> &str {
    match choices.default.as_str() {
        "claude" => "claude",
        "cursor" => "cursor-agent",
        "codex" => "codex",
        "opencode" => "opencode",
        "gemini" => "gemini",
        other if !other.is_empty() => other,
        _ => "copilot",
    }
}
