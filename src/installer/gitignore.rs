//! Additive `.gitignore` merge for files NodKray writes into a target repo.
//!
//! Existing lines are never deleted. Uninstall removes only the delimited
//! NodKray block, leaving any pre-existing MCP / tool ignore rules intact.

use std::path::Path;

use crate::error::NodkrayResult;

pub const BEGIN: &str = "# nodkray:gitignore-begin";
pub const END: &str = "# nodkray:gitignore-end";

const CORE_ENTRIES: &[&str] = &[".nodkray/", "skills/nodkray/"];
const OPTIONAL_ENTRIES: &[&str] = &[".serena/", ".codegraph/", ".sentrux/"];

/// Merge the NodKray ignore block. `created_agents_md` also ignores `AGENTS.md`
/// when NodKray created that file from scratch.
pub fn ensure_block(project_root: &Path, created_agents_md: bool) -> NodkrayResult<bool> {
    let path = project_root.join(".gitignore");
    let existing = if path.is_file() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };

    let block = render_block(&existing, created_agents_md);
    if let Some(start) = existing.find(BEGIN) {
        if let Some(end_rel) = existing[start..].find(END) {
            let end = start + end_rel + END.len();
            let mut after = existing[end..].to_string();
            if after.starts_with('\n') {
                after = after[1..].to_string();
            }
            let updated = format!("{}{}{}", &existing[..start], block, after);
            if updated == existing {
                return Ok(false);
            }
            std::fs::write(&path, updated)?;
            return Ok(true);
        }
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    if !next.is_empty() {
        next.push('\n');
    }
    next.push_str(&block);
    std::fs::write(&path, next)?;
    Ok(true)
}

/// Remove only the NodKray ignore block. Other rules stay.
pub fn remove_block(project_root: &Path) -> NodkrayResult<bool> {
    let path = project_root.join(".gitignore");
    if !path.is_file() {
        return Ok(false);
    }
    let existing = std::fs::read_to_string(&path)?;
    let Some(start) = existing.find(BEGIN) else {
        return Ok(false);
    };
    let Some(end_rel) = existing[start..].find(END) else {
        return Ok(false);
    };
    let end = start + end_rel + END.len();
    let mut next = existing[..start].to_string();
    let mut after = existing[end..].to_string();
    if after.starts_with('\n') {
        after = after[1..].to_string();
    }
    next.push_str(&after);
    while next.ends_with("\n\n\n") {
        next.pop();
    }
    if next.trim().is_empty() {
        std::fs::remove_file(&path)?;
    } else {
        std::fs::write(&path, next)?;
    }
    Ok(true)
}

fn outside_block(existing: &str) -> String {
    if let Some(start) = existing.find(BEGIN) {
        if let Some(end_rel) = existing[start..].find(END) {
            let end = start + end_rel + END.len();
            let mut text = existing[..start].to_string();
            text.push_str(&existing[end..]);
            return text;
        }
    }
    existing.to_string()
}

fn render_block(existing: &str, created_agents_md: bool) -> String {
    let outside = outside_block(existing);
    let present: Vec<&str> = outside
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    let mut entries: Vec<&str> = CORE_ENTRIES.to_vec();
    for entry in OPTIONAL_ENTRIES {
        if !present.iter().any(|line| line == entry) {
            entries.push(*entry);
        }
    }
    if created_agents_md && !present.iter().any(|line| *line == "AGENTS.md") {
        entries.push("AGENTS.md");
    }

    let mut out = String::new();
    out.push_str(BEGIN);
    out.push('\n');
    out.push_str("# NodKray\n");
    for entry in entries {
        out.push_str(entry);
        out.push('\n');
    }
    out.push_str(END);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_is_additive_and_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(".gitignore");
        std::fs::write(&path, "target/\n.serena/\n").expect("seed");

        assert!(ensure_block(tmp.path(), false).expect("first"));
        let once = std::fs::read_to_string(&path).expect("read");
        assert!(once.contains("target/"));
        assert_eq!(once.matches(".serena/").count(), 1);
        assert!(once.contains(".nodkray/"));

        assert!(!ensure_block(tmp.path(), false).expect("second"));
        assert_eq!(once, std::fs::read_to_string(&path).expect("read2"));
    }

    #[test]
    fn remove_block_keeps_user_rules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(".gitignore");
        std::fs::write(&path, "target/\n.serena/\n").expect("seed");
        ensure_block(tmp.path(), true).expect("add");
        assert!(remove_block(tmp.path()).expect("remove"));
        let after = std::fs::read_to_string(&path).expect("read");
        assert!(after.contains("target/"));
        assert!(after.contains(".serena/"));
        assert!(!after.contains(BEGIN));
        assert!(!after.contains(".nodkray/"));
    }
}
