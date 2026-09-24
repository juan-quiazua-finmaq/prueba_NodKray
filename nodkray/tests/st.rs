//! Fase 2 integration tests: ST end-to-end with a mock Generic worker, plus
//! effort gating, review policy, merge conflict handling, crash recovery and
//! cancellation.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OVERRIDE_ENV: &[&str] = &[
    "NODKRAY_AGENT_FRONTIER",
    "NODKRAY_EXECUTION_BACKEND",
    "NODKRAY_DECISION_PROVIDER",
    "NODKRAY_REVIEW_DEPTH",
    "NODKRAY_MEMORY_PATH",
    "NODKRAY_REMOTE_ENABLED",
    "NODKRAY_SECURITY_YOLO",
    "NODKRAY_PROJECT_NAME",
    "NODKRAY_GENERIC_COMMAND",
];

/// Mock worker that adds a marker function and commits it.
const MOCK_SUCCESS: &str = r#"#!/bin/sh
printf '\npub fn worker_marker() -> i32 { 42 }\n' >> src/lib.rs
git add -A
git commit -q -m "worker: add marker" >/dev/null 2>&1 || true
printf '{"status":"completed","summary":"added marker","changed_files":["src/lib.rs"],"tests_run":["cargo test"],"notes":[],"blocking_issues":[]}\n'
"#;

/// Mock worker that commits a failing test.
const MOCK_FAILING_TESTS: &str = r#"#!/bin/sh
printf '#[test]\nfn fails() { assert_eq!(1, 2); }\n' > src/lib.rs
git add -A
git commit -q -m "worker: failing test" >/dev/null 2>&1 || true
printf '{"status":"completed","summary":"wrote failing test","changed_files":["src/lib.rs"],"tests_run":[],"notes":[],"blocking_issues":[]}\n'
"#;

const CARGO_TOML: &str = "[package]\nname = \"dummy\"\nversion = \"0.0.0\"\nedition = \"2021\"\n";
const LIB_RS: &str =
    "pub fn base() -> i32 { 1 }\n\n#[cfg(test)]\nmod t { #[test] fn ok() { assert_eq!(super::base(), 1); } }\n";

struct StSandbox {
    dir: tempfile::TempDir,
    config_home: PathBuf,
    data_home: PathBuf,
    repo: PathBuf,
}

impl StSandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_home = dir.path().join("config").join("nodkray");
        let data_home = dir.path().join("data").join("nodkray");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("src")).expect("repo dirs");

        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "NodKray Test"]);
        std::fs::write(repo.join("Cargo.toml"), CARGO_TOML).expect("cargo toml");
        std::fs::write(repo.join("src").join("lib.rs"), LIB_RS).expect("lib rs");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "initial"]);

        Self {
            dir,
            config_home,
            data_home,
            repo,
        }
    }

    fn write_mock(&self, name: &str, script: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, script).expect("write mock");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
        path
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

    fn run_with_mock(&self, args: &[&str], mock: &Path) -> Output {
        self.cmd()
            .env("NODKRAY_GENERIC_COMMAND", mock)
            .args(args)
            .output()
            .expect("run nodkray")
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().expect("run nodkray")
    }

    fn read_repo_file(&self, relative: &str) -> String {
        std::fs::read_to_string(self.repo.join(relative)).expect("read repo file")
    }

    fn worktrees_dir(&self) -> PathBuf {
        self.repo.join(".nodkray").join("worktrees")
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("signal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn high_effort_description() -> &'static str {
    "Refactor architecture across multiple modules and redesign the database schema, migrate all services with breaking API changes for production"
}

#[test]
fn st_end_to_end_merges_and_persists() {
    let sb = StSandbox::new();
    let mock = sb.write_mock("mock-success.sh", MOCK_SUCCESS);

    let output = sb.run_with_mock(&["--json", "task", "add a worker marker function"], &mock);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));

    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["workflow"], "ST");
    assert_eq!(payload["status"], "MERGED");
    assert_eq!(payload["review"]["status"], "passed");
    assert_eq!(payload["review"]["depth"], "fast");
    assert_eq!(payload["merge"]["status"], "merged");
    assert_eq!(payload["workers"][0]["status"], "completed");
    assert!(payload["effort"].as_u64().is_some());
    assert!(payload["task_id"].as_str().expect("task id").starts_with("task_"));

    // The worker change only reaches main through the merge.
    assert!(sb.read_repo_file("src/lib.rs").contains("worker_marker"));
    assert!(sb.worktrees_dir().is_dir());

    // Inspect shows the event trail, including worker events.
    let task_id = payload["task_id"].as_str().expect("id").to_string();
    let inspect = sb.run(&["--json", "task", "inspect", &task_id]);
    assert_eq!(code(&inspect), 0);
    let inspect_json: serde_json::Value = serde_json::from_str(&stdout(&inspect)).expect("json");
    assert_eq!(inspect_json["task"]["status"], "MERGED");
    let events: Vec<String> = inspect_json["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["event"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(events.contains(&"task.merged".to_string()));
    assert!(events.iter().any(|event| event.starts_with("worker.")));
}

#[test]
fn high_effort_selects_odd_or_sdd() {
    let sb = StSandbox::new();
    let mock = sb.write_mock("mock-success.sh", MOCK_SUCCESS);

    let output = sb.run_with_mock(&["--json", "task", high_effort_description()], &mock);
    let payload: serde_json::Value =
        serde_json::from_str(&stdout(&output)).unwrap_or_else(|_| serde_json::json!({}));
    if code(&output) == 0 {
        assert!(payload["workflow"] == "ODD" || payload["workflow"] == "SDD");
    } else {
        // SDD without Spec-Kit is a dependency error, never a silent ST fallback.
        assert!(
            payload["code"] == "SPECKIT_NOT_FOUND" || payload["code"] == "CONSTITUTION_MISSING",
            "unexpected SDD error: {payload}"
        );
    }
}

#[test]
fn force_st_above_threshold_continues_with_warning() {
    let sb = StSandbox::new();
    let mock = sb.write_mock("mock-success.sh", MOCK_SUCCESS);

    let output = sb.run_with_mock(
        &["--json", "task", "--workflow", "st", high_effort_description()],
        &mock,
    );
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("forcing ST"),
        "stderr: {}",
        stderr(&output)
    );
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["status"], "MERGED");
}

#[test]
fn failing_tests_fail_review_and_do_not_merge() {
    let sb = StSandbox::new();
    let mock = sb.write_mock("mock-failing.sh", MOCK_FAILING_TESTS);

    let output = sb.run_with_mock(&["--json", "task", "add a worker marker function"], &mock);
    assert_eq!(code(&output), 7, "stderr: {}", stderr(&output));

    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["status"], "FAILED");
    assert_eq!(payload["review"]["status"], "failed");
    assert_eq!(payload["merge"]["status"], "skipped");

    // Main was never touched by the failing worker.
    assert!(sb.read_repo_file("src/lib.rs").contains("pub fn base"));
    assert!(!sb.read_repo_file("src/lib.rs").contains("fails()"));
}

#[test]
fn unsupported_review_depth_is_invalid_usage() {
    let sb = StSandbox::new();
    let output = sb.run(&["--json", "task", "--review", "banana", "something"]);
    assert_eq!(code(&output), 2, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["code"], "INVALID_REVIEW_DEPTH");
}

#[test]
fn running_task_survives_crash_and_is_reported_stale() {
    use nodkray::core::task::{create_task, update_task_status, NewTask, TaskStatus};
    use nodkray::memory::sqlite::SqliteMemoryRepository;
    use nodkray::memory::MemoryRepository;

    let sb = StSandbox::new();
    let root = sb.repo.canonicalize().expect("canonicalize");

    // Simulate a process that classified and started a worker, then died.
    let task_id = {
        let repo = SqliteMemoryRepository::open(&sb.data_home.join("memory.db")).expect("open");
        let project = repo
            .get_or_create_project(&root.to_string_lossy())
            .expect("project");
        let task =
            create_task(&repo, &NewTask::new(project.id, "crashed task", "desc")).expect("task");
        update_task_status(&repo, &task.id, TaskStatus::Running, "task.running", None)
            .expect("running");
        task.id
        // connection dropped == crash
    };

    let status = sb.run(&["--json", "status"]);
    assert_eq!(code(&status), 0, "stderr: {}", stderr(&status));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&status)).expect("json");
    let tasks = payload["tasks"].as_array().expect("tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], task_id);
    assert_eq!(tasks[0]["status"], "RUNNING");
    assert_eq!(tasks[0]["stale"], true);

    let inspect = sb.run(&["--json", "task", "inspect", &task_id]);
    assert_eq!(code(&inspect), 0);
    let payload: serde_json::Value = serde_json::from_str(&stdout(&inspect)).expect("json");
    let events: Vec<String> = payload["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["event"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(events.contains(&"task.created".to_string()));
    assert!(events.contains(&"task.running".to_string()));
}

#[test]
fn cancel_marks_task_cancelled() {
    use nodkray::core::task::{create_task, NewTask};
    use nodkray::memory::sqlite::SqliteMemoryRepository;
    use nodkray::memory::MemoryRepository;

    let sb = StSandbox::new();
    let root = sb.repo.canonicalize().expect("canonicalize");
    let task_id = {
        let repo = SqliteMemoryRepository::open(&sb.data_home.join("memory.db")).expect("open");
        let project = repo
            .get_or_create_project(&root.to_string_lossy())
            .expect("project");
        create_task(&repo, &NewTask::new(project.id, "to cancel", "desc"))
            .expect("task")
            .id
    };

    let output = sb.run(&["--json", "task", "cancel", &task_id]);
    assert_eq!(code(&output), 0, "stderr: {}", stderr(&output));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json");
    assert_eq!(payload["status"], "CANCELLED");
}

#[test]
fn merge_conflict_is_reported_and_git_stays_clean() {
    use nodkray::review::git::{is_clean, merge_branch, MergeStatus};

    let sb = StSandbox::new();
    let repo = &sb.repo;
    let base_branch = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .expect("branch")
            .stdout,
    )
    .expect("utf8")
    .trim()
    .to_string();

    // Diverging branch editing the same file.
    git(repo, &["checkout", "-q", "-b", "nodkray/other"]);
    std::fs::write(repo.join("src").join("lib.rs"), "pub fn other() {}\n").expect("write");
    git(repo, &["commit", "-aqm", "other"]);

    git(repo, &["checkout", "-q", &base_branch]);
    std::fs::write(repo.join("src").join("lib.rs"), "pub fn main() {}\n").expect("write");
    git(repo, &["commit", "-aqm", "main"]);

    let outcome = merge_branch(repo, "nodkray/other").expect("merge");
    assert_eq!(outcome.status, MergeStatus::Conflict);
    assert!(is_clean(repo).expect("clean"), "git must be left clean");
    assert!(
        !repo.join(".git").join("MERGE_HEAD").exists(),
        "no merge in progress"
    );
}
