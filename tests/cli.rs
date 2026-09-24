//! End-to-end tests driving the compiled `nodkray` binary.
//!
//! Each test runs the binary in a throwaway sandbox: `NODKRAY_CONFIG_HOME`,
//! `NODKRAY_DATA_HOME` and `HOME` point inside a `tempfile::TempDir`, so no real
//! user configuration is touched and tests stay independent.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_yaml::Value;

const OVERRIDE_ENV: &[&str] = &[
    "NODKRAY_AGENT_FRONTIER",
    "NODKRAY_EXECUTION_BACKEND",
    "NODKRAY_DECISION_PROVIDER",
    "NODKRAY_REVIEW_DEPTH",
    "NODKRAY_MEMORY_PATH",
    "NODKRAY_REMOTE_ENABLED",
    "NODKRAY_REMOTE_BIND",
    "NODKRAY_REMOTE_TOKEN",
    "NODKRAY_JEV_URL",
    "NODKRAY_SECURITY_YOLO",
    "NODKRAY_PROJECT_NAME",
    "NODKRAY_REPO",
    "NODKRAY_VERSION",
    "NODKRAY_PREFIX",
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

    fn cmd_in(&self, dir: &Path) -> Command {
        let mut cmd = self.cmd();
        cmd.current_dir(dir);
        cmd
    }

    fn run_in(&self, dir: &Path, args: &[&str]) -> Output {
        self.cmd_in(dir)
            .args(args)
            .output()
            .expect("failed to run nodkray")
    }

    /// Create a second repository sharing this sandbox's data home.
    fn other_repo(&self, name: &str) -> PathBuf {
        let repo = self.dir.path().join(name);
        std::fs::create_dir_all(repo.join(".git")).expect("git dir");
        repo
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
fn init_project_appends_agents_md_without_losing_user_text() {
    let sb = Sandbox::with_git();
    let agents_md = sb.repo.join("AGENTS.md");
    let original = "# AGENTS\n\nDo not modify me.\n";
    std::fs::write(&agents_md, original).expect("seed AGENTS.md");

    let output = sb.run(&["init", "--project", "--yes"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));

    let after = std::fs::read_to_string(&agents_md).expect("read AGENTS.md");
    assert!(after.contains("Do not modify me."), "user text must stay");
    assert!(after.contains("<!-- nodkray:begin -->"));
    assert!(sb.repo.join(".gitignore").is_file());
    assert!(sb.repo.join("skills").join("nodkray").join("test.md").is_file());
}

#[test]
fn uninstall_project_keeps_preexisting_mcp_config() {
    let sb = Sandbox::with_git();
    std::fs::create_dir_all(sb.repo.join(".serena")).expect("serena");
    std::fs::write(sb.repo.join(".serena").join("project.yml"), "keep: true\n").expect("mcp");
    std::fs::write(sb.repo.join("AGENTS.md"), "# AGENTS\n\nKeep me.\n").expect("agents");

    let init = sb.run(&["init", "--project", "--yes"]);
    assert_eq!(code(&init), 0, "stderr: {}", stderr(&init));
    assert!(sb.repo.join(".nodkray").is_dir());

    let output = sb.run(&["uninstall", "--project", "--yes"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert!(!sb.repo.join(".nodkray").exists());
    assert!(!sb.repo.join("skills").join("nodkray").exists());
    assert!(sb.repo.join(".serena").join("project.yml").is_file());
    let agents = std::fs::read_to_string(sb.repo.join("AGENTS.md")).expect("agents after");
    assert!(agents.contains("Keep me."));
    assert!(!agents.contains("<!-- nodkray:begin -->"));
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
fn memory_save_search_get_timeline_roundtrip() {
    let sb = Sandbox::new();

    let save = sb.run(&[
        "memory",
        "save",
        "--type",
        "decision",
        "--title",
        "SQLite is the local source of truth",
        "--content",
        "Use SQLite + FTS5 for persistent memory.",
        "--importance",
        "3",
    ]);
    assert_eq!(code(&save), 0, "stderr: {}", stderr(&save));
    let id = stdout(&save).trim().to_string();
    assert!(id.starts_with("memory_"), "unexpected id: {id}");

    let search = sb.run(&["memory", "search", "SQLite", "--json"]);
    assert_eq!(code(&search), 0, "stderr: {}", stderr(&search));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&search)).expect("json");
    let results = payload["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"], id);
    assert_eq!(results[0]["type"], "decision");
    assert!(
        results[0].get("content").is_none(),
        "search must not leak full content"
    );
    assert!(results[0]["preview"].as_str().is_some());

    let get = sb.run(&["memory", "get", &id]);
    assert_eq!(code(&get), 0, "stderr: {}", stderr(&get));
    assert!(stdout(&get).contains("SQLite + FTS5"));

    let timeline = sb.run(&["memory", "timeline", "--json"]);
    assert_eq!(code(&timeline), 0);
    let payload: serde_json::Value = serde_json::from_str(&stdout(&timeline)).expect("json");
    assert_eq!(payload["entries"].as_array().expect("entries").len(), 1);
}

#[test]
fn memory_search_without_results_exits_zero_with_message() {
    let sb = Sandbox::new();
    let output = sb.run(&["memory", "search", "zzzunlikelyterm"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(!text.trim().is_empty());
    assert!(text.contains("no memories found"));
}

#[test]
fn memory_search_is_project_isolated_and_global_crosses() {
    let sb = Sandbox::new();
    let other = sb.other_repo("other-repo");

    let save = sb.run(&[
        "memory",
        "save",
        "--type",
        "discovery",
        "--title",
        "alphamarker finding",
        "--content",
        "only visible inside the first project",
    ]);
    assert_eq!(code(&save), 0, "stderr: {}", stderr(&save));

    // Project-scoped search from another repo must not see it.
    let scoped = sb.run_in(&other, &["memory", "search", "alphamarker", "--json"]);
    assert_eq!(code(&scoped), 0, "stderr: {}", stderr(&scoped));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&scoped)).expect("json");
    assert_eq!(payload["count"], 0);

    // --global crosses project boundaries.
    let global = sb.run_in(
        &other,
        &["memory", "search", "alphamarker", "--global", "--json"],
    );
    assert_eq!(code(&global), 0, "stderr: {}", stderr(&global));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&global)).expect("json");
    assert_eq!(payload["count"], 1);
}

#[test]
fn project_inspect_reports_identity_rules_and_counts() {
    let sb = Sandbox::with_git();
    std::fs::write(sb.repo.join("AGENTS.md"), "project rules\n").expect("write AGENTS.md");

    let output = sb.run(&["project", "inspect", "--json"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");

    assert_eq!(payload["name"], "repo");
    assert!(payload["root"].as_str().expect("root").ends_with("repo"));
    let rules = payload["rules"].as_array().expect("rules");
    assert!(rules
        .iter()
        .any(|rule| rule["name"] == "AGENTS.md" && rule["kind"] == "agents"));
    assert_eq!(payload["counts"]["memories"], 0);
    assert_eq!(payload["counts"]["tasks"], 0);
}

#[test]
fn task_inspect_unknown_id_is_invalid_usage() {
    let sb = Sandbox::new();
    let output = sb.run(&["task", "inspect", "task_missing", "--json"]);
    assert_eq!(code(&output), 2, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["code"], "TASK_NOT_FOUND");
    assert_eq!(payload["category"], "user_input");
}

#[test]
fn status_and_task_inspect_read_persisted_tasks() {
    use nodkray::core::task::{create_task, NewTask};
    use nodkray::memory::sqlite::SqliteMemoryRepository;
    use nodkray::memory::MemoryRepository;

    let sb = Sandbox::new();
    // The CLI and the test must agree on the canonical project root.
    let root = sb.repo.canonicalize().expect("canonicalize");

    let task_id = {
        let repo = SqliteMemoryRepository::open(&sb.data_home.join("memory.db")).expect("open");
        let project = repo
            .get_or_create_project(&root.to_string_lossy())
            .expect("project");
        create_task(&repo, &NewTask::new(project.id, "persisted task", "desc"))
            .expect("create")
            .id
    };

    let status = sb.run(&["status", "--json"]);
    assert_eq!(code(&status), 0, "stderr: {}", stderr(&status));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&status)).expect("json");
    let tasks = payload["tasks"].as_array().expect("tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], task_id);
    assert_eq!(tasks[0]["status"], "PENDING");
    assert_eq!(tasks[0]["workflow"], "auto");
    assert!(tasks[0]["updated_at"].as_str().is_some());

    let inspect = sb.run(&["task", "inspect", &task_id, "--json"]);
    assert_eq!(code(&inspect), 0, "stderr: {}", stderr(&inspect));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&inspect)).expect("json");
    assert_eq!(payload["task"]["id"], task_id);
    assert_eq!(payload["task"]["status"], "PENDING");
    assert_eq!(payload["events"].as_array().expect("events").len(), 1);
}

#[test]
fn serve_without_token_exits_auth() {
    let sb = Sandbox::with_git();
    let output = sb.run(&["serve", "--json"]);
    assert_eq!(code(&output), 10, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["code"], "AUTH_TOKEN_MISSING");
    assert_eq!(payload["category"], "permission");
}

#[test]
fn remote_is_disabled_by_default() {
    let sb = Sandbox::new();
    let output = sb.run(&["config", "get", "remote.enabled", "--json"]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["value"], false);
}

