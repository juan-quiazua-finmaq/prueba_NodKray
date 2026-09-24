//! Idempotent AGENTS.md block (spec §93). Never destructive.

use std::path::Path;

use crate::error::NodkrayResult;

pub const BEGIN: &str = "<!-- nodkray:begin -->";
pub const END: &str = "<!-- nodkray:end -->";

const BLOCK: &str = r#"<!-- nodkray:begin -->
## NodKray

Use `nodkray memory search` before revisiting project decisions.

Use `nodkray task` for delegated work.

Do not modify another worker's worktree.

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
}
