//! Console execution backend (spec §15).
//!
//! Each worker is a separate process; file isolation is a git worktree (spec
//! §16). Spawning uses `std::process` and one stdin-writer thread so prompt
//! writes cannot deadlock against stdout/stderr capture.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use super::traits::{
    ExecutionBackend, WorkerEvent, WorkerOutcome, WorkerSpec, WorkerStatus, Workspace,
    WorkspaceRequest,
};
use crate::error::{NodkrayError, NodkrayResult};

/// Console backend: process spawn + git worktrees.
pub struct ConsoleBackend;

impl ConsoleBackend {
    pub fn new() -> Self {
        Self
    }

    /// `.nodkray/worktrees/<task-id>-<token>`.
    pub fn worktree_path(root: &Path, task_id: &str, token: &str) -> PathBuf {
        root.join(".nodkray")
            .join("worktrees")
            .join(format!("{task_id}-{token}"))
    }

    /// `nodkray/<task-id>-<token>`.
    pub fn branch_name(task_id: &str, token: &str) -> String {
        format!("nodkray/{task_id}-{token}")
    }
}

impl Default for ConsoleBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn git_error(code: &str, output: &std::process::Output) -> NodkrayError {
    NodkrayError::git(
        code,
        format!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    )
}

impl ExecutionBackend for ConsoleBackend {
    fn name(&self) -> &'static str {
        "console"
    }

    fn available(&self) -> bool {
        true
    }

    fn create_workspace(&self, request: &WorkspaceRequest) -> NodkrayResult<Workspace> {
        let path = Self::worktree_path(&request.root, &request.task_id, &request.token);
        let branch = Self::branch_name(&request.task_id, &request.token);

        if path.is_dir() {
            // Reuse an existing worktree (idempotent, cacheable; spec §16 rule 3).
            return Ok(Workspace {
                root: request.root.clone(),
                path,
                branch,
                created: false,
            });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                NodkrayError::execution(
                    "WORKTREE_CREATE_FAILED",
                    format!("could not create {}: {}", parent.display(), err),
                )
            })?;
        }

        let base = request.base_branch.as_deref().unwrap_or("HEAD");
        let path_str = path.to_string_lossy().to_string();
        let output = Command::new("git")
            .arg("-C")
            .arg(&request.root)
            .args(["worktree", "add", "-b", &branch, &path_str, base])
            .output()
            .map_err(|err| NodkrayError::git("GIT_UNAVAILABLE", err.to_string()))?;

        if !output.status.success() {
            return Err(git_error("WORKTREE_CREATE_FAILED", &output));
        }

        Ok(Workspace {
            root: request.root.clone(),
            path,
            branch,
            created: true,
        })
    }

    fn spawn_worker(
        &self,
        spec: &WorkerSpec,
        cwd: &Path,
        on_event: &mut dyn FnMut(WorkerEvent),
    ) -> NodkrayResult<WorkerOutcome> {
        on_event(WorkerEvent::new(
            "worker.created",
            json!({
                "task_id": spec.task_id,
                "role": spec.role,
                "agent": spec.agent,
                "worktree": spec.worktree.as_ref().map(|path| path.display().to_string()),
                "branch": spec.branch,
            }),
        ));

        if !cwd.is_dir() {
            on_event(WorkerEvent::new(
                "worker.failed",
                json!({ "reason": "working directory does not exist" }),
            ));
            return Err(NodkrayError::execution(
                "WORKER_CWD_MISSING",
                format!("working directory does not exist: {}", cwd.display()),
            ));
        }

        let mut command = Command::new(&spec.process.program);
        command
            .args(&spec.process.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &spec.process.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|err| {
            on_event(WorkerEvent::new(
                "worker.failed",
                json!({ "reason": format!("spawn failed: {err}") }),
            ));
            NodkrayError::execution(
                "WORKER_SPAWN_FAILED",
                format!("failed to spawn {}: {}", spec.process.program, err),
            )
        })?;

        let pid = child.id();
        on_event(WorkerEvent::new("worker.started", json!({ "pid": pid })));

        // Write the prompt on a thread; dropping stdin signals EOF.
        let stdin = child.stdin.take();
        let prompt = spec.process.stdin.clone();
        let writer = std::thread::spawn(move || {
            if let Some(mut pipe) = stdin {
                if let Some(prompt) = prompt {
                    let _ = pipe.write_all(prompt.as_bytes());
                }
            }
        });

        let output = child.wait_with_output().map_err(|err| {
            NodkrayError::execution("WORKER_WAIT_FAILED", err.to_string())
        })?;
        let _ = writer.join();

        let status = if output.status.success() {
            WorkerStatus::Completed
        } else {
            WorkerStatus::Failed
        };
        let exit_code = output.status.code();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        let (event, payload) = match status {
            WorkerStatus::Completed => (
                "worker.completed",
                json!({ "exit_code": exit_code, "stdout_tail": tail(&stdout) }),
            ),
            WorkerStatus::Failed => (
                "worker.failed",
                json!({ "exit_code": exit_code, "stderr_tail": tail(&stderr) }),
            ),
        };
        on_event(WorkerEvent::new(event, payload));

        Ok(WorkerOutcome {
            status,
            exit_code,
            stdout,
            stderr,
        })
    }
}

fn tail(text: &str) -> String {
    const MAX: usize = 500;
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    let start = trimmed.chars().count() - MAX;
    trimmed.chars().skip(start).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::ProcessSpec;

    #[test]
    fn worktree_and_branch_naming() {
        let root = Path::new("/repo");
        assert_eq!(
            ConsoleBackend::worktree_path(root, "task_1", "default"),
            PathBuf::from("/repo/.nodkray/worktrees/task_1-default")
        );
        assert_eq!(
            ConsoleBackend::branch_name("task_1", "default"),
            "nodkray/task_1-default"
        );
    }

    #[test]
    fn spawn_captures_stdout_and_emits_events() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = WorkerSpec {
            task_id: "task_1".to_string(),
            role: "default".to_string(),
            agent: "generic".to_string(),
            worktree: None,
            branch: None,
            process: ProcessSpec {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), "printf hello".to_string()],
                stdin: None,
                env: Vec::new(),
            },
        };
        let mut events = Vec::new();
        let backend = ConsoleBackend::new();
        let outcome = backend
            .spawn_worker(&spec, tmp.path(), &mut |event| events.push(event.event))
            .expect("spawn");

        assert_eq!(outcome.status, WorkerStatus::Completed);
        assert!(outcome.stdout.contains("hello"));
        assert_eq!(events[0], "worker.created");
        assert!(events.contains(&"worker.started".to_string()));
        assert!(events.contains(&"worker.completed".to_string()));
    }

    #[test]
    fn nonzero_exit_is_failed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = WorkerSpec {
            task_id: "task_1".to_string(),
            role: "default".to_string(),
            agent: "generic".to_string(),
            worktree: None,
            branch: None,
            process: ProcessSpec {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), "exit 3".to_string()],
                stdin: None,
                env: Vec::new(),
            },
        };
        let backend = ConsoleBackend::new();
        let outcome = backend
            .spawn_worker(&spec, tmp.path(), &mut |_| {})
            .expect("spawn");
        assert_eq!(outcome.status, WorkerStatus::Failed);
        assert_eq!(outcome.exit_code, Some(3));
    }
}
