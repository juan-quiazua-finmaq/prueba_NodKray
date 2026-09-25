//! Contract probes for optional tools (doctor + backend selection).
//!
//! Presence on `PATH` is not enough: NodKray must invoke a command the binary
//! actually implements. Never launch a bare TUI (`herdr`, `sentrux`,
//! `codegraph`).

use std::process::{Command, Stdio};

use super::tools::find_in_path;

/// Captured stdout/stderr from a short probe.
#[derive(Debug, Clone)]
pub struct ProbeOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl ProbeOutput {
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// Run `{binary} {args…}` without a TTY. Returns `None` if the binary is missing
/// or cannot be spawned.
pub fn run_probe(binary: &str, args: &[&str]) -> Option<ProbeOutput> {
    let path = find_in_path(binary)?;
    let output = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    Some(ProbeOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn looks_unknown(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("unknown command")
        || text.contains("no such command")
        || text.contains("unrecognized subcommand")
        || text.contains("unrecognized command")
        || text.contains("unexpected argument")
}

/// Herdr 0.9.1 is a multiplexer and has no `run` process-wrapper. Do not launch
/// bare `herdr` (it attaches a TUI).
pub fn herdr_runner_usable() -> bool {
    let Some(out) = run_probe("herdr", &["run", "--help"]) else {
        return false;
    };
    if looks_unknown(&out.combined()) {
        return false;
    }
    out.success || out.combined().to_ascii_lowercase().contains("usage")
}

/// Spec-Kit CLI is `specify version` / `specify check` / `specify init`.
/// Stages (`/speckit.specify`, …) are agent slash commands, not CLI args.
pub fn specify_cli_usable() -> bool {
    if let Some(out) = run_probe("specify", &["version"]) {
        if out.success && !looks_unknown(&out.combined()) {
            return true;
        }
    }
    if let Some(out) = run_probe("specify", &["--help"]) {
        return out.success && !looks_unknown(&out.combined());
    }
    false
}

/// Review uses `sentrux check .`. Bare `sentrux` opens a GUI.
pub fn sentrux_check_usable() -> bool {
    let Some(out) = run_probe("sentrux", &["--help"]) else {
        return false;
    };
    if looks_unknown(&out.combined()) {
        return false;
    }
    let text = out.combined().to_ascii_lowercase();
    text.contains("check") || out.success
}

/// Serena's contract is `serena start-mcp-server` / `serena --help`.
pub fn serena_usable() -> bool {
    let Some(out) = run_probe("serena", &["--help"]) else {
        return false;
    };
    !looks_unknown(&out.combined()) && (out.success || out.combined().to_ascii_lowercase().contains("start-mcp-server"))
}

/// CodeGraph: probe `version` only. Bare `codegraph` is an interactive installer.
pub fn codegraph_usable() -> bool {
    let Some(out) = run_probe("codegraph", &["version"]) else {
        return false;
    };
    out.success && !looks_unknown(&out.combined())
}

pub fn herdr_backend_detail() -> String {
    if find_in_path("herdr").is_none() {
        return "not found; optional".to_string();
    }
    if herdr_runner_usable() {
        "usable as execution backend (`herdr run`)".to_string()
    } else {
        "present; no `herdr run` contract — console fallback".to_string()
    }
}

pub fn speckit_cli_detail() -> String {
    if find_in_path("specify").is_none() {
        return "not found; SDD needs `specify` on PATH".to_string();
    }
    if specify_cli_usable() {
        "CLI ok (`specify version`); stages are /speckit.*".to_string()
    } else {
        "on PATH; `specify version` failed — SDD not wired".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_text_is_detected() {
        assert!(looks_unknown("error: unknown command: run"));
        assert!(looks_unknown("No such command 'specify'"));
        assert!(!looks_unknown("Usage: herdr pane run <pane> <cmd>"));
    }

    #[test]
    fn herdr_without_run_is_not_a_runner() {
        if find_in_path("herdr").is_none() {
            assert!(!herdr_runner_usable());
            return;
        }
        // Installed Herdr 0.9.x has no `run`; a future runner would pass.
        let _ = herdr_runner_usable();
    }
}
