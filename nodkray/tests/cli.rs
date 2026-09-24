//! End-to-end tests driving the compiled `nodkray` binary.
//!
//! Each test runs the binary in a throwaway sandbox: `NODKRAY_CONFIG_HOME`,
//! `NODKRAY_DATA_HOME` and `HOME` point inside a `tempfile::TempDir`, so no real
//! user configuration is touched and tests stay independent.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_yaml::Value;

const OVERRIDE_ENV: &[&str] = &[
    "NODKRAY_AGENT_FRONTIER",
    "NODKRAY_EXECUTION_BACKEND",
    "NODKRAY_DECISION_PROVIDER",
    "NODKRAY_REVIEW_DEPTH",
    "NODKRAY_MEMORY_PATH",
    "NODKRAY_REMOTE_ENABLED",
    "NODKRAY_SECURITY_YOLO",
    "NODKRAY_PROJECT_NAME",
];

struct Sandbox {
    dir: tempfile::TempDir,
    config_home: PathBuf,
    data_home: PathBuf,
    repo: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_home = dir.path().join("config").join("nodkray");
        let data_home = dir.path().join("data").join("nodkray");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        Self {
            dir,
            config_home,
            data_home,
            repo,
        }
    }

    fn with_git() -> Self {
        let sb = Self::new();
        std::fs::create_dir_all(sb.repo.join(".git")).expect("git dir");
        sb
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_nodkray"));
        cmd.env("NODKRAY_CONFIG_HOME", &self.config_home)
            .env("NODKRAY_DATA_HOME", &self.data_home)
            .env("HOME", self.dir.path())
            .current_dir(&self.repo);
        for key in OVERRIDE_ENV {
            cmd.env_remove(key);
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().expect("failed to run nodkray")
    }

    fn global_config(&self) -> PathBuf {
        self.config_home.join("config.yaml")
    }

    fn project_config(&self) -> PathBuf {
        self.repo.join(".nodkray").join("config.yaml")
    }
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("process was killed by a signal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn init_project_twice_is_idempotent() {
    let sb = Sandbox::with_git();

    let first = sb.run(&["init", "--project"]);
    assert_eq!(code(&first), 0, "stderr: {}", stderr(&first));
    let first_bytes = std::fs::read(sb.project_config()).expect("config after first init");

    let second = sb.run(&["init", "--project"]);
    assert_eq!(code(&second), 0, "stderr: {}", stderr(&second));
    let second_bytes = std::fs::read(sb.project_config()).expect("config after second init");

    assert_eq!(
        first_bytes, second_bytes,
        "init must not rewrite an existing config"
    );

    let first_value: Value = serde_yaml::from_slice(&first_bytes).expect("parse first config");
    let second_value: Value = serde_yaml::from_slice(&second_bytes).expect("parse second config");
    assert_eq!(first_value, second_value);

    // Directory layout is created.
    assert!(sb.repo.join(".nodkray").join("tasks").is_dir());
    assert!(sb.repo.join(".nodkray").join("worktrees").is_dir());
}

#[test]
fn init_project_does_not_touch_existing_agents_md() {
    let sb = Sandbox::with_git();
    let agents_md = sb.repo.join("AGENTS.md");
    let original = b"# AGENTS\n\nDo not modify me.\n";
    std::fs::write(&agents_md, original).expect("seed AGENTS.md");

    let output = sb.run(&["init", "--project"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));

    let after = std::fs::read(&agents_md).expect("read AGENTS.md");
    assert_eq!(after, original, "init must not modify project rules");

    // The rules are only detected and reported.
    assert!(stdout(&output).contains("AGENTS.md"));
}

#[test]
fn doctor_reports_optional_tools_without_failing() {
    let sb = Sandbox::with_git();
    let output = sb.run(&["doctor", "--json"]);
    let text = stdout(&output);

    let report: serde_json::Value = serde_json::from_str(&text).expect("doctor --json must be JSON");
    let checks = report["checks"].as_array().expect("checks array");

    // Herdr (and the other integrations) must never be reported as missing.
    for name in ["Herdr", "Spec-Kit", "Sentrux", "Serena", "CodeGraph"] {
        let check = checks
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("missing check for {name}"));
        assert_ne!(
            check["status"], "missing",
            "{name} is optional and must not be 'missing'"
        );
    }
    assert!(checks.iter().any(|c| c["name"] == "SQLite"));

    let git_available = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if git_available {
        // Optional tools absent must not fail the process.
        assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    } else {
        assert_eq!(code(&output), 4);
    }
}

#[test]
fn config_get_set_roundtrip() {
    let sb = Sandbox::new();

    let set = sb.run(&["config", "set", "review.default_depth", "deep"]);
    assert_eq!(code(&set), 0, "stderr: {}", stderr(&set));

    let get = sb.run(&["config", "get", "review.default_depth", "--json"]);
    assert_eq!(code(&get), 0, "stderr: {}", stderr(&get));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&get)).expect("json payload");
    assert_eq!(payload["key"], "review.default_depth");
    assert_eq!(payload["value"], "deep");

    // Setting again is stable and does not duplicate keys.
    let set_again = sb.run(&["config", "set", "review.default_depth", "deep"]);
    assert_eq!(code(&set_again), 0);
    let text = std::fs::read_to_string(sb.global_config()).expect("config file");
    assert_eq!(
        text.matches("default_depth").count(),
        1,
        "key must appear exactly once: {text}"
    );

    // No project config exists, so the write went to the global file.
    assert!(sb.global_config().is_file());
    assert!(!sb.project_config().exists());
}

#[test]
fn config_get_unknown_key_is_invalid_usage_in_json() {
    let sb = Sandbox::new();
    let output = sb.run(&["config", "get", "does.not.exist", "--json"]);
    assert_eq!(code(&output), 2);

    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("error json");
    assert_eq!(payload["code"], "CONFIG_KEY_NOT_FOUND");
    assert_eq!(payload["category"], "user_input");
}

#[test]
fn unknown_command_exits_2() {
    let sb = Sandbox::new();
    let output = sb.run(&["definitely-not-a-command"]);
    assert_eq!(code(&output), 2);
}

#[test]
fn corrupt_config_exits_3() {
    let sb = Sandbox::new();
    std::fs::create_dir_all(&sb.config_home).expect("config home");
    std::fs::write(sb.global_config(), "version: 1\n  bad: [unterminated\n").expect("write corrupt");

    let output = sb.run(&["config", "get", "version", "--json"]);
    assert_eq!(code(&output), 3, "stderr: {}", stderr(&output));

    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("error json");
    assert_eq!(payload["category"], "configuration");
    assert_eq!(payload["recoverable"], false);
}

#[test]
fn status_is_empty_but_valid() {
    let sb = Sandbox::new();

    let text = sb.run(&["status"]);
    assert_eq!(code(&text), 0);
    assert!(stdout(&text).contains("no active tasks"));

    let json = sb.run(&["status", "--json"]);
    assert_eq!(code(&json), 0);
    let payload: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("valid json");
    assert_eq!(payload["tasks"], serde_json::json!([]));
}

#[test]
fn agent_list_json_includes_registry() {
    let sb = Sandbox::new();
    let output = sb.run(&["agent", "list", "--json"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    let ids: Vec<_> = payload
        .as_array()
        .expect("array")
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&"claude".to_string()));
    assert!(ids.contains(&"generic".to_string()));
}

#[test]
fn task_run_odd_writes_task_md() {
    let sb = Sandbox::with_git();
    assert_eq!(code(&sb.run(&["init", "--project"])), 0);
    let output = sb.run(&["task", "run", "Add JWT auth", "--workflow", "odd", "--json"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["workflow"], "ODD");
    let artefact = payload["artefact"].as_str().expect("artefact");
    let text = std::fs::read_to_string(artefact).expect("task.md");
    assert!(text.contains("## Objective"));
    assert!(text.contains("Add JWT auth"));
}

#[test]
fn mcp_list_json_is_valid() {
    let sb = Sandbox::with_git();
    assert_eq!(code(&sb.run(&["init", "--project"])), 0);
    let output = sb.run(&["mcp", "list", "--json"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert!(payload.as_array().expect("array").iter().any(|s| s["name"] == "serena"));
}

#[test]
fn skills_install_is_idempotent() {
    let sb = Sandbox::with_git();
    assert_eq!(code(&sb.run(&["init", "--project"])), 0);
    let first = sb.run(&["skills", "--agents-md"]);
    assert_eq!(code(&first), 0, "stderr: {}", stderr(&first));
    assert!(sb.repo.join("skills/nodkray/memory.md").is_file());
    let agents = std::fs::read_to_string(sb.repo.join("AGENTS.md")).expect("agents");
    assert!(agents.contains("<!-- nodkray:begin -->"));
    let second = sb.run(&["skills", "--agents-md"]);
    assert_eq!(code(&second), 0);
    let again = std::fs::read_to_string(sb.repo.join("AGENTS.md")).expect("agents2");
    assert_eq!(agents.matches("## NodKray").count(), 1);
    assert_eq!(agents, again);
}
