//! Role -> agent assignment (spec §12).

use crate::config::Config;

/// Provider configured for a worker role.
pub fn worker_provider<'a>(config: &'a Config, role: &str) -> &'a str {
    match role {
        "backend" => &config.agents.workers.backend.provider,
        "frontend" => &config.agents.workers.frontend.provider,
        "reviewer" => &config.agents.workers.reviewer.provider,
        "docs" => &config.agents.workers.docs.provider,
        _ => &config.agents.workers.default.provider,
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
