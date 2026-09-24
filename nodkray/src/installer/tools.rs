//! Tool detection on `PATH` and via project markers (spec §63, §69).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Look up an executable on `PATH` (a small `which`-like helper).
pub fn find_in_path(executable: &str) -> Option<PathBuf> {
    let raw = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&raw) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(executable);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "cmd", "bat", "ps1"] {
                let candidate = dir.join(format!("{executable}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// A detected external tool/integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolDetection {
    pub name: String,
    pub found: bool,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ToolDetection {
    fn new(name: &str, found: bool, required: bool, path: Option<PathBuf>, detail: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            found,
            required,
            path,
            detail,
        }
    }
}

/// Detect the tools NodKray cares about. Each integration is satisfied by an
/// executable on `PATH` or by a project-local marker directory.
pub fn detect_tools(root: Option<&Path>) -> Vec<ToolDetection> {
    let mut tools = Vec::new();

    let git = find_in_path("git");
    tools.push(ToolDetection::new("Git", git.is_some(), true, git, None));

    tools.push(binary_or_marker(
        "Herdr",
        "herdr",
        root.map(|r| r.join(".herdr")),
    ));
    tools.push(ToolDetection::new(
        "Spec-Kit",
        find_in_path("specify").is_some() || marker_exists(root, ".specify"),
        false,
        find_in_path("specify"),
        marker_detail(root, ".specify"),
    ));
    tools.push(binary_or_marker(
        "Sentrux",
        "sentrux",
        root.map(|r| r.join(".sentrux")),
    ));
    tools.push(ToolDetection::new(
        "Serena",
        find_in_path("serena").is_some(),
        false,
        find_in_path("serena"),
        None,
    ));
    tools.push(ToolDetection::new(
        "CodeGraph",
        find_in_path("codegraph").is_some(),
        false,
        find_in_path("codegraph"),
        None,
    ));

    tools
}

fn binary_or_marker(name: &str, binary: &str, marker: Option<PathBuf>) -> ToolDetection {
    let path = find_in_path(binary);
    let marker = marker.filter(|m| m.exists());
    let found = path.is_some() || marker.is_some();
    ToolDetection::new(
        name,
        found,
        false,
        path,
        marker.map(|m| format!("marker: {}", m.display())),
    )
}

fn marker_exists(root: Option<&Path>, marker: &str) -> bool {
    root.map(|r| r.join(marker).exists()).unwrap_or(false)
}

fn marker_detail(root: Option<&Path>, marker: &str) -> Option<String> {
    marker_exists(root, marker).then(|| format!("marker: {}", marker))
}

/// Convenience used by the doctor.
pub fn find_tool<'a>(tools: &'a [ToolDetection], name: &str) -> Option<&'a ToolDetection> {
    tools.iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_tools_without_panicking() {
        let tools = detect_tools(None);
        assert!(find_tool(&tools, "Git").is_some());
        assert!(find_tool(&tools, "Herdr").is_some());
    }

    #[test]
    fn project_marker_counts_as_detected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".specify")).expect("marker");
        let tools = detect_tools(Some(tmp.path()));
        assert!(find_tool(&tools, "Spec-Kit").expect("speckit").found);
    }
}
