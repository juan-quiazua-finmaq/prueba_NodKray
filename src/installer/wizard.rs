//! Interactive agent-role assignment for `nodkray init`.

use serde::Serialize;

use crate::config::{set_in_file, Config};
use crate::error::NodkrayResult;
use crate::installer::agents::{detect_agents, KNOWN_AGENTS};
use crate::installer::prompt::{is_interactive, prompt_choice, prompt_line};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentChoices {
    pub frontier: String,
    pub default: String,
    pub backend: String,
    pub frontend: String,
    pub reviewer: String,
    pub docs: String,
    pub generic_command: Option<String>,
}

impl AgentChoices {
    pub fn from_defaults() -> Self {
        let defaults = Config::default();
        Self {
            frontier: defaults.agents.frontier.provider,
            default: defaults.agents.workers.default.provider,
            backend: defaults.agents.workers.backend.provider,
            frontend: defaults.agents.workers.frontend.provider,
            reviewer: defaults.agents.workers.reviewer.provider,
            docs: defaults.agents.workers.docs.provider,
            generic_command: None,
        }
    }

    pub fn from_detected() -> Self {
        let mut choices = Self::from_defaults();
        let found: Vec<String> = detect_agents()
            .into_iter()
            .filter(|a| a.found)
            .map(|a| a.name)
            .collect();
        if let Some(first) = found.first() {
            choices.frontier = first.clone();
        }
        for name in &found {
            if name == "codex" {
                choices.default = name.clone();
                choices.backend = name.clone();
            }
            if name == "cursor" {
                choices.frontend = name.clone();
            }
            if name == "claude" {
                choices.reviewer = name.clone();
            }
            if name == "pi" {
                choices.docs = name.clone();
            }
        }
        choices
    }
}

/// Ask (or default) which provider covers each role.
pub fn collect_choices(interactive: bool, yes: bool) -> NodkrayResult<AgentChoices> {
    let mut choices = AgentChoices::from_detected();
    if !interactive || yes {
        return Ok(choices);
    }

    let mut options: Vec<String> = detect_agents()
        .into_iter()
        .filter(|a| a.found)
        .map(|a| a.name)
        .collect();
    for (name, _) in KNOWN_AGENTS {
        if !options.iter().any(|o| o == name) {
            options.push((*name).to_string());
        }
    }
    options.push("generic".to_string());

    choices.frontier = prompt_choice("Frontier agent", &options, &choices.frontier)?;
    choices.default = prompt_choice("Worker (default)", &options, &choices.default)?;
    choices.backend = prompt_choice("Worker (backend)", &options, &choices.backend)?;
    choices.frontend = prompt_choice("Worker (frontend)", &options, &choices.frontend)?;
    choices.reviewer = prompt_choice("Worker (reviewer)", &options, &choices.reviewer)?;
    choices.docs = prompt_choice("Worker (docs)", &options, &choices.docs)?;

    let uses_generic = [
        &choices.frontier,
        &choices.default,
        &choices.backend,
        &choices.frontend,
        &choices.reviewer,
        &choices.docs,
    ]
    .iter()
    .any(|p| *p == "generic");
    if uses_generic {
        let command = prompt_line("Generic worker command: ", "")?;
        if !command.is_empty() {
            choices.generic_command = Some(command);
        }
    }
    Ok(choices)
}

/// Write role providers without clobbering values the user already customized.
pub fn apply_choices(
    config_path: &std::path::Path,
    existing: Option<&Config>,
    choices: &AgentChoices,
    reconfigure: bool,
) -> NodkrayResult<Vec<String>> {
    let defaults = Config::default();
    let mut changed = Vec::new();

    let assignments = [
        (
            "agents.frontier.provider",
            &choices.frontier,
            existing.map(|c| c.agents.frontier.provider.as_str()),
            defaults.agents.frontier.provider.as_str(),
        ),
        (
            "agents.workers.default.provider",
            &choices.default,
            existing.map(|c| c.agents.workers.default.provider.as_str()),
            defaults.agents.workers.default.provider.as_str(),
        ),
        (
            "agents.workers.backend.provider",
            &choices.backend,
            existing.map(|c| c.agents.workers.backend.provider.as_str()),
            defaults.agents.workers.backend.provider.as_str(),
        ),
        (
            "agents.workers.frontend.provider",
            &choices.frontend,
            existing.map(|c| c.agents.workers.frontend.provider.as_str()),
            defaults.agents.workers.frontend.provider.as_str(),
        ),
        (
            "agents.workers.reviewer.provider",
            &choices.reviewer,
            existing.map(|c| c.agents.workers.reviewer.provider.as_str()),
            defaults.agents.workers.reviewer.provider.as_str(),
        ),
        (
            "agents.workers.docs.provider",
            &choices.docs,
            existing.map(|c| c.agents.workers.docs.provider.as_str()),
            defaults.agents.workers.docs.provider.as_str(),
        ),
    ];

    for (key, value, current, default) in assignments {
        let should_write = reconfigure
            || current.is_none()
            || current == Some(default)
            || current == Some(value.as_str());
        if should_write && current != Some(value.as_str()) {
            set_in_file(config_path, key, value)?;
            changed.push(key.to_string());
        }
    }

    if let Some(command) = &choices.generic_command {
        let current = existing.and_then(|c| c.agents.generic.command.as_deref());
        if reconfigure || current.is_none() {
            set_in_file(config_path, "agents.generic.command", command)?;
            changed.push("agents.generic.command".to_string());
        }
    }

    let _ = is_interactive;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{write_value, ConfigPaths};

    #[test]
    fn apply_does_not_clobber_custom_providers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("cfg"), tmp.path().join("data"));
        let path = paths.global_config_file();
        let mut existing = Config::default();
        existing.agents.frontier.provider = "claude".to_string();
        write_value(&path, &serde_yaml::to_value(&existing).expect("yaml")).expect("write");

        let mut choices = AgentChoices::from_defaults();
        choices.frontier = "codex".to_string();
        let changed = apply_choices(&path, Some(&existing), &choices, false).expect("apply");
        assert!(!changed.iter().any(|k| k == "agents.frontier.provider"));

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains("claude"));
        assert!(!text.contains("frontier:\n  provider: codex"));
    }

    #[test]
    fn non_interactive_collect_does_not_need_a_tty() {
        let choices = collect_choices(false, true).expect("choices");
        assert!(!choices.frontier.is_empty());
    }
}
