//! Idempotent AGENTS.md block (spec §93). Never destructive.

use std::path::Path;

use crate::error::NodkrayResult;

pub const BEGIN: &str = "<!-- nodkray:begin -->";
pub const END: &str = "<!-- nodkray:end -->";

const BLOCK: &str = r#"<!-- nodkray:begin -->
## NodKray

ST = small change. ODD = medium change (`task.md`). SDD = Spec-Kit (`/speckit.*`).

Use `nodkray task --workflow st|odd|sdd` for delegated work. Stay inside the
current working directory (the task worktree). Do not edit another worker's
worktree or the repo root.

Use `nodkray memory search` or `nodkray memory timeline` before revisiting
project decisions. `memory get <id>` accepts memory and decision ids.

Use `nodkray review` before requesting merge.
<!-- nodkray:end -->
"#;

/// Insert or refresh the delimited NodKray section. Other content is preserved.
pub fn ensure_block(project_root: &Path) -> NodkrayResult<bool> {
    let path = project_root.join("AGENTS.md");
    let existing = if path.is_file() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };

    if let Some(start) = existing.find(BEGIN) {
        if let Some(end_rel) = existing[start..].find(END) {
            let end = start + end_rel + END.len();
            let mut after = existing[end..].to_string();
            if after.starts_with('\n') {
                after = after[1..].to_string();
            }
            let updated = format!("{}{}{}", &existing[..start], BLOCK, after);
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
    next.push_str(BLOCK);
    std::fs::write(&path, next)?;
    Ok(true)
}

/// Remove only the delimited NodKray section. User text stays.
pub fn remove_block(project_root: &Path) -> NodkrayResult<bool> {
    let path = project_root.join("AGENTS.md");
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
        std::fs::write(&path, "")?;
    } else {
        std::fs::write(&path, next)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_is_idempotent_and_preserves_user_text() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("AGENTS.md");
        std::fs::write(&path, "# Project\n\nKeep me.\n").expect("write");

        assert!(ensure_block(tmp.path()).expect("first"));
        let once = std::fs::read_to_string(&path).expect("read");
        assert!(once.contains("Keep me."));
        assert_eq!(once.matches(BEGIN).count(), 1);

        assert!(!ensure_block(tmp.path()).expect("second"));
        let twice = std::fs::read_to_string(&path).expect("read2");
        assert_eq!(once, twice);
        assert_eq!(twice.matches("## NodKray").count(), 1);
    }

    #[test]
    fn remove_block_preserves_user_text() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("AGENTS.md");
        std::fs::write(&path, "# Project\n\nKeep me.\n").expect("write");
        ensure_block(tmp.path()).expect("insert");
        assert!(remove_block(tmp.path()).expect("remove"));
        let after = std::fs::read_to_string(&path).expect("read");
        assert!(after.contains("Keep me."));
        assert!(!after.contains(BEGIN));
    }
}
