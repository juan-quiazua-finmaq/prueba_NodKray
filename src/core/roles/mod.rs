//! Role -> agent assignment (spec §12, §95).

use crate::config::schema::AgentConfig;
use crate::config::Config;

/// Provider configured for a worker role.
pub fn worker_provider<'a>(config: &'a Config, role: &str) -> &'a str {
    match role {
        "backend" => &config.agents.workers.backend.provider,
        "frontend" => &config.agents.workers.frontend.provider,
        "reviewer" => &config.agents.workers.reviewer.provider,
        "docs" | "documentation" => &config.agents.workers.docs.provider,
        "frontier" => &config.agents.frontier.provider,
        _ => &config.agents.workers.default.provider,
    }
}

/// MCP selection for a role: `all`, `none`, or an explicit list.
pub fn mcp_for_role(config: &Config, role: &str) -> Vec<String> {
    role_config(config, role).mcp.clone()
}

fn role_config<'a>(config: &'a Config, role: &'a str) -> &'a AgentConfig {
    match role.trim().to_ascii_lowercase().as_str() {
        "frontier" => &config.agents.frontier,
        "backend" => &config.agents.workers.backend,
        "frontend" => &config.agents.workers.frontend,
        "reviewer" => &config.agents.workers.reviewer,
        "docs" | "documentation" => &config.agents.workers.docs,
        _ => &config.agents.workers.default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_and_named_roles() {
        let config = Config::default();
        assert_eq!(worker_provider(&config, "default"), "codex");
        assert_eq!(worker_provider(&config, "frontend"), "cursor");
        assert_eq!(worker_provider(&config, "unknown-role"), "codex");
    }
}
