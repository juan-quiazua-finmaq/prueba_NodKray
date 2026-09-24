//! Config schema (spec §61-§62).
//!
//! Every struct implements `Default`, so any subset of the YAML document can be
//! merged in cascade (defaults < user < project < env < CLI) and deserialised
//! with `#[serde(default)]`.

use serde::{Deserialize, Serialize};

/// Current on-disk config version.
pub const CONFIG_VERSION: u32 = 1;

/// Default memory database path (expanded at use time).
pub const DEFAULT_MEMORY_PATH: &str = "~/.nodkray/memory.db";

/// Default local control API bind address.
pub const DEFAULT_REMOTE_BIND: &str = "127.0.0.1:8787";

/// Root configuration document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub project: ProjectConfig,
    pub agents: AgentsConfig,
    pub execution: ExecutionConfig,
    pub decision: DecisionConfig,
    pub review: ReviewConfig,
    pub integrations: IntegrationsConfig,
    pub memory: MemoryConfig,
    pub remote: RemoteConfig,
    pub security: SecurityConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            project: ProjectConfig::default(),
            agents: AgentsConfig::default(),
            execution: ExecutionConfig::default(),
            decision: DecisionConfig::default(),
            review: ReviewConfig::default(),
            integrations: IntegrationsConfig::default(),
            memory: MemoryConfig::default(),
            remote: RemoteConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Config {
    /// Logical configuration snapshot stored at session start (spec §164).
    ///
    /// Later config edits must not change a historical session, so the snapshot
    /// is serialised once when the session is created.
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// `project` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectConfig {
    /// Project name; resolved from the repository directory name when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `agents` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub frontier: AgentConfig,
    pub workers: WorkersConfig,
    /// Generic adapter configuration (§129): arbitrary worker command.
    pub generic: GenericAgentConfig,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            frontier: AgentConfig::provider("opencode"),
            workers: WorkersConfig::default(),
            generic: GenericAgentConfig::default(),
        }
    }
}

/// `agents.generic` section (spec §129).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GenericAgentConfig {
    /// Command line executed as the generic worker, e.g. `my-agent --run`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// A role -> provider assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub provider: String,
    /// MCP servers available to this role (`all`, `none`, or explicit ids).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp: Vec<String>,
}

impl AgentConfig {
    pub fn provider(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            mcp: Vec::new(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self::provider("opencode")
    }
}

/// `agents.workers` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkersConfig {
    pub default: AgentConfig,
    pub backend: AgentConfig,
    pub frontend: AgentConfig,
    pub reviewer: AgentConfig,
    pub docs: AgentConfig,
}

impl Default for WorkersConfig {
    fn default() -> Self {
        Self {
            default: AgentConfig::provider("codex"),
            backend: AgentConfig::provider("codex"),
            frontend: AgentConfig::provider("cursor"),
            reviewer: AgentConfig::provider("claude"),
            docs: AgentConfig::provider("pi"),
        }
    }
}

/// `execution` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionConfig {
    pub backend: String,
    /// When Herdr is configured but missing, continue with Console (spec §166).
    pub fallback_console: bool,
    pub worktrees: WorktreesConfig,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            backend: "herdr".to_string(),
            fallback_console: true,
            worktrees: WorktreesConfig::default(),
        }
    }
}

/// `execution.worktrees` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreesConfig {
    pub enabled: bool,
}

impl Default for WorktreesConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// `decision` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecisionConfig {
    pub provider: String,
    pub thresholds: ThresholdsConfig,
    /// Environment variable that holds the JEV API key (spec §72). Never stored in SQLite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Optional JEV HTTP endpoint (`http://host:port/path`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            thresholds: ThresholdsConfig::default(),
            api_key_env: None,
            url: None,
        }
    }
}

impl DecisionConfig {
    /// Env var name for the JEV API key. Defaults to `JEV_API_KEY`.
    pub fn jev_api_key_env(&self) -> &str {
        self.api_key_env.as_deref().unwrap_or("JEV_API_KEY")
    }
}

/// `decision.thresholds` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThresholdsConfig {
    pub st_max: u32,
    pub odd_max: u32,
}

impl Default for ThresholdsConfig {
    fn default() -> Self {
        Self {
            st_max: 20,
            odd_max: 60,
        }
    }
}

/// `review` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewConfig {
    pub default_depth: String,
    pub policy: ReviewPolicyConfig,
    pub thresholds: ReviewThresholdsConfig,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            default_depth: "balanced".to_string(),
            policy: ReviewPolicyConfig::default(),
            thresholds: ReviewThresholdsConfig::default(),
        }
    }
}

/// `review.thresholds` section (spec §32).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewThresholdsConfig {
    pub fast_max: u32,
    pub balanced_max: u32,
}

impl Default for ReviewThresholdsConfig {
    fn default() -> Self {
        Self {
            fast_max: 20,
            balanced_max: 60,
        }
    }
}

/// `review.policy` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewPolicyConfig {
    pub test_failure: String,
    pub constitution_violation: String,
    pub sentrux_failure: String,
    pub lint_failure: String,
    pub reviewer_failure: String,
}

impl Default for ReviewPolicyConfig {
    fn default() -> Self {
        Self {
            test_failure: "block".to_string(),
            constitution_violation: "block".to_string(),
            sentrux_failure: "block".to_string(),
            lint_failure: "configurable".to_string(),
            reviewer_failure: "block".to_string(),
        }
    }
}

/// `integrations` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    pub speckit: ToggleConfig,
    pub sentrux: SentruxConfig,
    pub serena: ToggleConfig,
    pub codegraph: ToggleConfig,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            speckit: ToggleConfig::default(),
            sentrux: SentruxConfig::default(),
            serena: ToggleConfig::default(),
            codegraph: ToggleConfig::default(),
        }
    }
}

/// Generic `{ enabled: bool }` integration toggle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToggleConfig {
    pub enabled: bool,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// `integrations.sentrux` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SentruxConfig {
    pub enabled: bool,
    pub required: bool,
}

impl Default for SentruxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            required: false,
        }
    }
}

/// `memory` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub backend: String,
    pub path: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: "sqlite".to_string(),
            path: DEFAULT_MEMORY_PATH.to_string(),
        }
    }
}

/// `remote` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteConfig {
    pub enabled: bool,
    pub bind: String,
    /// Environment variable that holds the control-API token. Never stored in SQLite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: DEFAULT_REMOTE_BIND.to_string(),
            token_env: None,
        }
    }
}

impl RemoteConfig {
    /// Env var name for the control-API token. Defaults to `NODKRAY_REMOTE_TOKEN`.
    pub fn token_env_name(&self) -> &str {
        self.token_env.as_deref().unwrap_or("NODKRAY_REMOTE_TOKEN")
    }
}

/// `security` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SecurityConfig {
    pub yolo: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec_62() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.agents.frontier.provider, "opencode");
        assert_eq!(config.agents.workers.default.provider, "codex");
        assert_eq!(config.agents.workers.frontend.provider, "cursor");
        assert_eq!(config.execution.backend, "herdr");
        assert!(config.execution.fallback_console);
        assert!(config.execution.worktrees.enabled);
        assert_eq!(config.decision.provider, "local");
        assert_eq!(config.decision.thresholds.st_max, 20);
        assert_eq!(config.decision.thresholds.odd_max, 60);
        assert_eq!(config.review.default_depth, "balanced");
        assert_eq!(config.review.policy.test_failure, "block");
        assert!(config.integrations.speckit.enabled);
        assert!(!config.integrations.sentrux.required);
        assert_eq!(config.memory.backend, "sqlite");
        assert!(!config.remote.enabled);
        assert_eq!(config.remote.bind, "127.0.0.1:8787");
        assert!(!config.security.yolo);
    }

    #[test]
    fn partial_yaml_fills_missing_fields_with_defaults() {
        let config: Config = serde_yaml::from_str("version: 1\nsecurity:\n  yolo: true\n")
            .expect("partial config should parse");
        assert!(config.security.yolo);
        assert_eq!(config.agents.frontier.provider, "opencode");
    }

    #[test]
    fn unknown_fields_are_ignored_for_forward_compat() {
        let config: Config =
            serde_yaml::from_str("version: 1\nfuture_key: 42\n").expect("unknown keys ignored");
        assert_eq!(config.version, 1);
    }
}
