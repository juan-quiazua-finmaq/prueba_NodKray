//! Configuration loading and mutation (spec §61, §126, §159).
//!
//! Resolution order, highest precedence last:
//!
//! ```text
//! defaults < user config < project config < NODKRAY_* env < CLI args
//! ```
//!
//! Layering happens on `serde_yaml::Value` before deserialisation so that a
//! partially populated file never resets unset fields.

pub mod schema;

use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::error::{NodkrayError, NodkrayResult};

pub use schema::Config;
pub use schema::UpdateConfig;

/// Environment variable overriding the config home (`~/.config/nodkray`).
pub const ENV_CONFIG_HOME: &str = "NODKRAY_CONFIG_HOME";
/// Environment variable overriding the data home (`~/.nodkray`).
pub const ENV_DATA_HOME: &str = "NODKRAY_DATA_HOME";

/// Well-known filesystem locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    /// Config home, e.g. `~/.config/nodkray`.
    pub config_home: PathBuf,
    /// Data home, e.g. `~/.nodkray`.
    pub data_home: PathBuf,
}

impl ConfigPaths {
    /// Build paths from the environment, honouring explicit overrides first and
    /// falling back to the platform directories.
    pub fn from_env() -> Self {
        if let (Ok(config_home), Ok(data_home)) = (
            std::env::var(ENV_CONFIG_HOME),
            std::env::var(ENV_DATA_HOME),
        ) {
            if !config_home.is_empty() && !data_home.is_empty() {
                return Self::with_home(config_home, data_home);
            }
        }

        // Individual overrides still apply even if only one is set.
        let base_dirs = directories::BaseDirs::new();
        let home = base_dirs
            .as_ref()
            .map(|b| b.home_dir().to_path_buf())
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));

        let config_home = std::env::var(ENV_CONFIG_HOME)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("", "", "nodkray")
                    .map(|p| p.config_dir().to_path_buf())
            })
            .unwrap_or_else(|| home.join(".config").join("nodkray"));

        let data_home = std::env::var(ENV_DATA_HOME)
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".nodkray"));

        Self::with_home(config_home, data_home)
    }

    /// Explicit constructor, primarily for tests and dependency injection.
    pub fn with_home(config_home: impl Into<PathBuf>, data_home: impl Into<PathBuf>) -> Self {
        Self {
            config_home: config_home.into(),
            data_home: data_home.into(),
        }
    }

    /// `~/.config/nodkray/config.yaml`.
    pub fn global_config_file(&self) -> PathBuf {
        self.config_home.join("config.yaml")
    }

    /// `<root>/.nodkray/config.yaml`.
    pub fn project_config_file(&self, root: &Path) -> PathBuf {
        root.join(".nodkray").join("config.yaml")
    }

    /// `<root>/.nodkray`.
    pub fn project_dir(&self, root: &Path) -> PathBuf {
        root.join(".nodkray")
    }

    /// `~/.nodkray/logs`.
    pub fn logs_dir(&self) -> PathBuf {
        self.data_home.join("logs")
    }

    /// `~/.nodkray`.
    pub fn data_dir(&self) -> &Path {
        &self.data_home
    }

    /// Resolve the configured memory database path.
    ///
    /// Paths under `~/.nodkray/` follow an overridden data home (important for
    /// tests and sandboxes); any other path is expanded normally.
    pub fn resolve_memory_path(&self, configured: &str) -> PathBuf {
        if let Some(rest) = configured.strip_prefix("~/.nodkray/") {
            return self.data_home.join(rest);
        }
        if configured == "~/.nodkray" {
            return self.data_home.clone();
        }
        expand_home(configured)
    }
}

/// Parse a `NODKRAY_*` override value into a YAML scalar (bool/int/float/string).
pub fn parse_scalar(raw: &str) -> Value {
    if let Ok(v) = raw.parse::<bool>() {
        return Value::Bool(v);
    }
    if let Ok(v) = raw.parse::<i64>() {
        return Value::Number(v.into());
    }
    Value::String(raw.to_string())
}

/// The mapping of supported env vars to dotted config keys (spec §126).
pub const ENV_OVERRIDES: &[(&str, &str)] = &[
    ("NODKRAY_AGENT_FRONTIER", "agents.frontier.provider"),
    ("NODKRAY_EXECUTION_BACKEND", "execution.backend"),
    ("NODKRAY_DECISION_PROVIDER", "decision.provider"),
    ("NODKRAY_REVIEW_DEPTH", "review.default_depth"),
    ("NODKRAY_MEMORY_PATH", "memory.path"),
    ("NODKRAY_REMOTE_ENABLED", "remote.enabled"),
    ("NODKRAY_REMOTE_BIND", "remote.bind"),
    ("NODKRAY_JEV_URL", "decision.url"),
    ("NODKRAY_SECURITY_YOLO", "security.yolo"),
    ("NODKRAY_PROJECT_NAME", "project.name"),
    ("NODKRAY_GENERIC_COMMAND", "agents.generic.command"),
];

/// Default configuration as a YAML value.
pub fn default_value() -> Value {
    value_of(&Config::default())
}

fn value_of(config: &Config) -> Value {
    serde_yaml::to_value(config).unwrap_or(Value::Mapping(Default::default()))
}

/// Deep-merge `overlay` into `base`. Mappings recurse; any other value replaces.
pub fn deep_merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(slot) => deep_merge(slot, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base_slot, overlay_value) => *base_slot = overlay_value,
    }
}

/// Read a YAML mapping from `path`. Missing file yields `Ok(None)`; malformed
/// content yields a `configuration` error.
pub fn read_layer(path: &Path) -> NodkrayResult<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            if text.trim().is_empty() {
                return Ok(Some(Value::Mapping(Default::default())));
            }
            let value: Value = serde_yaml::from_str(&text).map_err(|err| {
                NodkrayError::configuration(
                    "CONFIG_PARSE_ERROR",
                    format!("{}: {}", path.display(), err),
                )
            })?;
            match value {
                Value::Mapping(_) => Ok(Some(value)),
                Value::Null => Ok(Some(Value::Mapping(Default::default()))),
                _ => Err(NodkrayError::configuration(
                    "CONFIG_NOT_MAPPING",
                    format!("{}: expected a YAML mapping at the top level", path.display()),
                )),
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(NodkrayError::configuration(
            "CONFIG_READ_ERROR",
            format!("{}: {}", path.display(), err),
        )),
    }
}

/// Apply `NODKRAY_*` overrides onto a merged value.
pub fn apply_env_overrides(value: &mut Value) {
    for (env_key, config_key) in ENV_OVERRIDES {
        if let Ok(raw) = std::env::var(env_key) {
            set_path(value, config_key, parse_scalar(&raw));
        }
    }
}

/// Load the effective configuration for `project_root` (if any).
pub fn load_effective(paths: &ConfigPaths, project_root: Option<&Path>) -> NodkrayResult<Config> {
    let mut merged = default_value();

    if let Some(user) = read_layer(&paths.global_config_file())? {
        deep_merge(&mut merged, user);
    }
    if let Some(root) = project_root {
        if let Some(project) = read_layer(&paths.project_config_file(root))? {
            deep_merge(&mut merged, project);
        }
    }

    apply_env_overrides(&mut merged);

    let mut config: Config = serde_yaml::from_value(merged)
        .map_err(|err| NodkrayError::configuration("CONFIG_INVALID", err.to_string()))?;

    if config.project.name.is_none() {
        if let Some(root) = project_root {
            config.project.name = root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
        }
    }

    Ok(config)
}

/// Load the effective configuration as a raw YAML value (used by `config get`).
pub fn load_effective_value(
    paths: &ConfigPaths,
    project_root: Option<&Path>,
) -> NodkrayResult<Value> {
    let config = load_effective(paths, project_root)?;
    Ok(value_of(&config))
}

/// Load only defaults + global file, ignoring project config and env.
pub fn load_global_value(paths: &ConfigPaths) -> NodkrayResult<Value> {
    let mut merged = default_value();
    if let Some(user) = read_layer(&paths.global_config_file())? {
        deep_merge(&mut merged, user);
    }
    Ok(merged)
}

/// Look up a dotted key inside a YAML mapping.
pub fn get_path<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in key.split('.').filter(|s| !s.is_empty()) {
        let map = current.as_mapping()?;
        current = map.get(Value::String(segment.to_string()))?;
    }
    Some(current)
}

/// Assign `new_value` at a dotted key, creating intermediate mappings.
pub fn set_path(value: &mut Value, key: &str, new_value: Value) {
    let segments: Vec<&str> = key.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        *value = new_value;
        return;
    }
    if !value.is_mapping() {
        *value = Value::Mapping(Default::default());
    }

    let mut current = value;
    for segment in &segments[..segments.len() - 1] {
        // `current` is a mapping by construction (coerced above / on insertion).
        let Some(map) = current.as_mapping_mut() else {
            return;
        };
        let entry = map
            .entry(Value::String((*segment).to_string()))
            .or_insert_with(|| Value::Mapping(Default::default()));
        if !entry.is_mapping() {
            *entry = Value::Mapping(Default::default());
        }
        current = entry;
    }

    let last = segments[segments.len() - 1].to_string();
    if let Some(map) = current.as_mapping_mut() {
        map.insert(Value::String(last), new_value);
    }
}

/// Serialise and write a YAML value atomically-ish (create + truncate).
pub fn write_value(path: &Path, value: &Value) -> NodkrayResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            NodkrayError::configuration(
                "CONFIG_WRITE_ERROR",
                format!("could not create {}: {}", parent.display(), err),
            )
        })?;
    }
    let text = serde_yaml::to_string(value).map_err(|err| {
        NodkrayError::configuration("CONFIG_SERIALIZE_ERROR", err.to_string())
    })?;
    std::fs::write(path, text).map_err(|err| {
        NodkrayError::configuration(
            "CONFIG_WRITE_ERROR",
            format!("could not write {}: {}", path.display(), err),
        )
    })
}

/// Set a dotted key inside `path`, preserving the rest of the file.
pub fn set_in_file(path: &Path, key: &str, raw_value: &str) -> NodkrayResult<Value> {
    let mut value = read_layer(path)?.unwrap_or_else(|| Value::Mapping(Default::default()));
    let parsed = if raw_value.is_empty() {
        Value::String(String::new())
    } else {
        serde_yaml::from_str::<Value>(raw_value).unwrap_or_else(|_| Value::String(raw_value.to_string()))
    };
    set_path(&mut value, key, parsed.clone());
    write_value(path, &value)?;
    Ok(parsed)
}

/// Expand a leading `~` to the current user home directory.
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(base) = directories::BaseDirs::new() {
            return base.home_dir().join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml(text: &str) -> Value {
        serde_yaml::from_str(text).expect("valid yaml")
    }

    #[test]
    fn deep_merge_overrides_scalars_and_keeps_untouched_keys() {
        let mut base = yaml("a:\n  b: 1\n  c: keep\n");
        deep_merge(&mut base, yaml("a:\n  b: 2\n"));
        assert_eq!(get_path(&base, "a.b").and_then(Value::as_i64), Some(2));
        assert_eq!(
            get_path(&base, "a.c").and_then(Value::as_str),
            Some("keep")
        );
    }

    #[test]
    fn set_path_creates_intermediate_mappings() {
        let mut value = Value::Mapping(Default::default());
        set_path(&mut value, "execution.backend", Value::String("console".into()));
        assert_eq!(
            get_path(&value, "execution.backend").and_then(Value::as_str),
            Some("console")
        );
    }

    #[test]
    fn parse_scalar_understands_common_types() {
        assert_eq!(parse_scalar("true"), Value::Bool(true));
        assert_eq!(parse_scalar("20").as_i64(), Some(20));
        assert_eq!(parse_scalar("balanced").as_str(), Some("balanced"));
    }

    #[test]
    fn cascade_prefers_project_over_global() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(&root).expect("repo dir");

        write_value(
            &paths.global_config_file(),
            &yaml("version: 1\nexecution:\n  backend: console\n"),
        )
        .expect("write global");
        write_value(
            &paths.project_config_file(&root),
            &yaml("version: 1\nexecution:\n  backend: herdr\n"),
        )
        .expect("write project");

        let config = load_effective(&paths, Some(&root)).expect("load");
        assert_eq!(config.execution.backend, "herdr");
        // Untouched global field still flows through.
        assert_eq!(config.decision.thresholds.st_max, 20);
    }

    #[test]
    fn corrupt_config_is_a_configuration_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        std::fs::create_dir_all(paths.config_home.clone()).expect("config home");
        std::fs::write(paths.global_config_file(), "version: [unterminated").expect("write");
        let err = load_effective(&paths, None).expect_err("must fail");
        assert_eq!(err.category(), crate::error::ErrorCategory::Configuration);
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn project_name_falls_back_to_directory_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let root = tmp.path().join("my-repo");
        std::fs::create_dir_all(&root).expect("repo dir");
        let config = load_effective(&paths, Some(&root)).expect("load");
        assert_eq!(config.project.name.as_deref(), Some("my-repo"));
    }
}
