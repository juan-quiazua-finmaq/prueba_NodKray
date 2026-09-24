//! Optional tool provisioning. Failures are reported, never fatal to `init`.
//!
//! Uninstall never reverses these installs: MCP servers, Spec-Kit and Herdr
//! stay on the machine even if NodKray put them there.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::installer::tools::find_in_path;
use crate::spec::speckit::SpecKitAdapter;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvisionAction {
    pub name: String,
    pub attempted: bool,
    pub installed: bool,
    pub skipped: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ProvisionReport {
    pub actions: Vec<ProvisionAction>,
}

impl ProvisionReport {
    fn push(&mut self, action: ProvisionAction) {
        self.actions.push(action);
    }
}

/// Install missing known MCP servers. Already-present tools are skipped.
pub fn provision_mcps(report: &mut ProvisionReport, allow_network: bool) {
    provision_serena(report, allow_network);
    provision_codegraph(report, allow_network);
    provision_sentrux(report, allow_network);
}

/// Warn and install the latest Spec-Kit release from GitHub when `specify` is missing.
pub fn provision_speckit(report: &mut ProvisionReport, allow_network: bool) {
    if SpecKitAdapter::detect(None).executable.is_some() {
        report.push(already("spec-kit", "specify already on PATH"));
        return;
    }
    if !allow_network {
        report.push(skip(
            "spec-kit",
            "specify not found; SDD will fail until Spec-Kit is installed",
        ));
        return;
    }
    if ensure_uv(report, allow_network).is_none() {
        report.push(fail(
            "spec-kit",
            "uv is required to install specify-cli from github/spec-kit",
        ));
        return;
    }
    let tag = latest_github_tag("github", "spec-kit").unwrap_or_else(|| "v0.16.1".to_string());
    let from = format!("git+https://github.com/github/spec-kit.git@{tag}");
    match run_cmd("uv", &["tool", "install", "specify-cli", "--from", &from]) {
        Ok(detail) => report.push(ok("spec-kit", format!("installed {tag}: {detail}"))),
        Err(detail) => report.push(fail("spec-kit", detail)),
    }
}

/// Install Herdr only when the user asked. Never uninstalls it later.
pub fn provision_herdr(report: &mut ProvisionReport, allow_network: bool) {
    if find_in_path("herdr").is_some() {
        report.push(already("herdr", "herdr already on PATH"));
        return;
    }
    if !allow_network {
        report.push(skip("herdr", "not installed; Console fallback remains available"));
        return;
    }
    let result = if cfg!(windows) {
        run_cmd(
            "powershell",
            &[
                "-ExecutionPolicy",
                "Bypass",
                "-c",
                "irm https://herdr.dev/install.ps1 | iex",
            ],
        )
    } else {
        run_shell("curl -fsSL https://herdr.dev/install.sh | sh")
    };
    match result {
        Ok(detail) => report.push(ok("herdr", detail)),
        Err(detail) => report.push(fail("herdr", detail)),
    }
}

fn provision_serena(report: &mut ProvisionReport, allow_network: bool) {
    if find_in_path("serena").is_some() {
        report.push(already("serena", "already on PATH"));
        return;
    }
    if !allow_network {
        report.push(skip("serena", "not found"));
        return;
    }
    if ensure_uv(report, allow_network).is_none() {
        report.push(fail("serena", "uv is required (`uv tool install serena-agent`)"));
        return;
    }
    match run_cmd("uv", &["tool", "install", "-p", "3.13", "serena-agent"]) {
        Ok(detail) => report.push(ok("serena", detail)),
        Err(detail) => report.push(fail("serena", detail)),
    }
}

fn provision_codegraph(report: &mut ProvisionReport, allow_network: bool) {
    if find_in_path("codegraph").is_some() {
        report.push(already("codegraph", "already on PATH"));
        return;
    }
    if !allow_network {
        report.push(skip("codegraph", "not found"));
        return;
    }
    if find_in_path("npm").is_none() {
        report.push(fail("codegraph", "npm is required (`npm i -g @colbymchenry/codegraph`)"));
        return;
    }
    match run_cmd("npm", &["i", "-g", "@colbymchenry/codegraph"]) {
        Ok(detail) => report.push(ok("codegraph", detail)),
        Err(detail) => report.push(fail("codegraph", detail)),
    }
}

fn provision_sentrux(report: &mut ProvisionReport, allow_network: bool) {
    if find_in_path("sentrux").is_some() {
        report.push(already("sentrux", "already on PATH"));
        return;
    }
    if !allow_network {
        report.push(skip("sentrux", "not found"));
        return;
    }
    if find_in_path("brew").is_none() {
        report.push(skip(
            "sentrux",
            "Homebrew not found; install from https://github.com/sentrux/sentrux",
        ));
        return;
    }
    match run_cmd("brew", &["install", "sentrux/tap/sentrux"]) {
        Ok(detail) => report.push(ok("sentrux", detail)),
        Err(detail) => report.push(fail("sentrux", detail)),
    }
}

fn ensure_uv(report: &mut ProvisionReport, allow_network: bool) -> Option<std::path::PathBuf> {
    if let Some(path) = find_in_path("uv") {
        return Some(path);
    }
    if !allow_network {
        report.push(skip("uv", "not found"));
        return None;
    }
    let result = if cfg!(windows) {
        run_cmd(
            "powershell",
            &["-ExecutionPolicy", "Bypass", "-c", "irm https://astral.sh/uv/install.ps1 | iex"],
        )
    } else {
        run_shell("curl -fsSL https://astral.sh/uv/install.sh | sh")
    };
    match result {
        Ok(detail) => {
            report.push(ok("uv", detail));
            find_in_path("uv")
        }
        Err(detail) => {
            report.push(fail("uv", detail));
            None
        }
    }
}

fn latest_github_tag(owner: &str, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let output = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", &url])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    json.get("tag_name")?.as_str().map(|s| s.to_string())
}

fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("{program}: {err}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn run_shell(script: &str) -> Result<String, String> {
    let output = Command::new("sh")
        .args(["-c", script])
        .output()
        .map_err(|err| err.to_string())?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn already(name: &str, detail: impl Into<String>) -> ProvisionAction {
    ProvisionAction {
        name: name.to_string(),
        attempted: false,
        installed: true,
        skipped: true,
        detail: detail.into(),
    }
}

fn skip(name: &str, detail: impl Into<String>) -> ProvisionAction {
    ProvisionAction {
        name: name.to_string(),
        attempted: false,
        installed: false,
        skipped: true,
        detail: detail.into(),
    }
}

fn ok(name: &str, detail: impl Into<String>) -> ProvisionAction {
    ProvisionAction {
        name: name.to_string(),
        attempted: true,
        installed: true,
        skipped: false,
        detail: detail.into(),
    }
}

fn fail(name: &str, detail: impl Into<String>) -> ProvisionAction {
    ProvisionAction {
        name: name.to_string(),
        attempted: true,
        installed: false,
        skipped: false,
        detail: detail.into(),
    }
}

/// Names that were newly installed (not pre-existing).
pub fn newly_installed(report: &ProvisionReport) -> Vec<String> {
    report
        .actions
        .iter()
        .filter(|a| a.attempted && a.installed)
        .map(|a| a.name.clone())
        .collect()
}

/// Project-local tool markers must never be created just to "satisfy" detect.
pub fn has_project_marker(root: &Path, marker: &str) -> bool {
    root.join(marker).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_network_skips_missing_tools() {
        let mut report = ProvisionReport::default();
        provision_mcps(&mut report, false);
        provision_speckit(&mut report, false);
        provision_herdr(&mut report, false);
        assert!(report.actions.iter().all(|a| a.skipped || a.installed));
        assert!(newly_installed(&report).is_empty());
    }
}
