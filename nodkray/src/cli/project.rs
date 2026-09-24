//! `nodkray project inspect` (spec §66, §159).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::json;

use crate::cli::Context;
use crate::error::NodkrayResult;
use crate::installer::project::detect_rules;

/// Arguments for `nodkray project`.
#[derive(Debug, Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// Show project identity, detected rules and record counts.
    Inspect(InspectArgs),
}

/// Arguments for `nodkray project inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {}

/// A detected rule enriched with a stable kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RuleInfo {
    name: String,
    path: String,
    kind: String,
}

/// Run `nodkray project`.
pub fn run(ctx: &Context, args: ProjectArgs) -> NodkrayResult<i32> {
    match args.command {
        ProjectCommand::Inspect(_) => inspect(ctx),
    }
}

fn inspect(ctx: &Context) -> NodkrayResult<i32> {
    let repo = ctx.open_memory()?;
    let project = ctx.memory_project(&repo)?;
    let root = ctx.project_root().root;

    let mut rules = Vec::new();
    for rule in detect_rules(&root) {
        let kind = rule_kind(&rule.name).to_string();
        let hash = file_hash(Path::new(&rule.path));
        repo.upsert_project_rule(&project.id, &rule.path, &kind, hash.as_deref())?;
        rules.push(RuleInfo {
            name: rule.name,
            path: rule.path,
            kind,
        });
    }

    let stats = repo.project_stats(&project.id)?;

    if ctx.output.json {
        ctx.output.emit_json(&json!({
            "id": project.id,
            "name": project.name,
            "root": project.root_path,
            "git_remote": project.git_remote,
            "rules": rules,
            "counts": stats,
        }));
    } else {
        ctx.output.emit_text(format!("Project: {}", project.name));
        ctx.output.emit_text(format!("Root: {}", project.root_path));
        ctx.output.emit_text(format!(
            "Git remote: {}",
            project.git_remote.as_deref().unwrap_or("-")
        ));
        if rules.is_empty() {
            ctx.output.emit_text("Rules: none detected");
        } else {
            ctx.output.emit_text("Rules:");
            for rule in &rules {
                ctx.output
                    .emit_text(format!("  [{}] {}  ({})", rule.kind, rule.name, rule.path));
            }
        }
        ctx.output.emit_text(format!(
            "Counts: memories={} tasks={} decisions={} observations={} sessions={} reviews={}",
            stats.memories,
            stats.tasks,
            stats.decisions,
            stats.observations,
            stats.sessions,
            stats.reviews
        ));
    }

    Ok(0)
}

/// Classify a rule file by its name (spec §66).
fn rule_kind(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("constitution") {
        "constitution"
    } else if lower.ends_with("agents.md") || lower.ends_with("agents.local.md") {
        "agents"
    } else if lower.ends_with("claude.md") {
        "claude"
    } else if lower.ends_with("rules.toml") {
        "sentrux"
    } else if lower.ends_with("readme.md") {
        "readme"
    } else {
        "rule"
    }
}

/// Non-cryptographic content hash used to detect rule changes.
fn file_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::rule_kind;

    #[test]
    fn classifies_rule_kinds() {
        assert_eq!(rule_kind("CONSTITUTION.md"), "constitution");
        assert_eq!(rule_kind(".specify/memory/constitution.md"), "constitution");
        assert_eq!(rule_kind("AGENTS.md"), "agents");
        assert_eq!(rule_kind("CLAUDE.md"), "claude");
        assert_eq!(rule_kind(".sentrux/rules.toml"), "sentrux");
        assert_eq!(rule_kind("other.txt"), "rule");
    }
}
