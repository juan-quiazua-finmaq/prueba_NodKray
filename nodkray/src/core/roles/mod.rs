//! Role → agent assignment (spec §12, §95).

use crate::config::schema::{AgentConfig, Config};

/// Resolve the provider configured for a worker role.
pub fn agent_for_role(config: &Config, role: &str) -> String {
    role_config(config, role).provider.clone()
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
    fn maps_roles_to_default_providers() {
        let config = Config::default();
        assert_eq!(agent_for_role(&config, "frontier"), "opencode");
        assert_eq!(agent_for_role(&config, "backend"), "codex");
        assert_eq!(agent_for_role(&config, "frontend"), "cursor");
        assert_eq!(agent_for_role(&config, "reviewer"), "claude");
        assert_eq!(agent_for_role(&config, "docs"), "pi");
        assert_eq!(agent_for_role(&config, "unknown"), "codex");
    }
}
