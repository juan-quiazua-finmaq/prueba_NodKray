//! NodKray skills for frontier agents (spec §92). Idempotent install.

use std::path::Path;

use crate::error::NodkrayResult;

const SKILLS: &[(&str, &str)] = &[
    (
        "memory.md",
        "# NodKray memory\n\nUse `nodkray memory search` before revisiting project decisions.\nUse `nodkray memory get <id>` for full content. Do not open SQLite directly.\n",
    ),
    (
        "task.md",
        "# NodKray tasks\n\nUse `nodkray task run` for delegated work.\nPass `--workflow odd|sdd` when the decision engine is not choosing.\nDo not modify another worker's worktree.\n",
    ),
    (
        "review.md",
        "# NodKray review\n\nUse `nodkray review` before requesting merge.\n`--review fast|balanced|deep` overrides automatic depth.\nA missing mandatory check is BLOCKED, never a silent pass.\n",
    ),
    (
        "orchestration.md",
        "# NodKray orchestration\n\nRoles select agents. Execution goes through `nodkray` backends (Herdr or console).\nInspect workers with `nodkray worker list`. Cancel with `nodkray task cancel`.\n",
    ),
];

pub fn skills_dir(project_root: &Path) -> std::path::PathBuf {
    project_root.join("skills").join("nodkray")
}

/// Install skills without overwriting user edits.
pub fn install_into(project_root: &Path) -> NodkrayResult<Vec<String>> {
    let dir = skills_dir(project_root);
    std::fs::create_dir_all(&dir)?;
    let mut written = Vec::new();
    for (name, body) in SKILLS {
        let path = dir.join(name);
        if path.exists() {
            continue;
        }
        std::fs::write(&path, body)?;
        written.push(name.to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = install_into(tmp.path()).expect("first");
        assert_eq!(first.len(), 4);
        std::fs::write(skills_dir(tmp.path()).join("memory.md"), "custom\n").expect("edit");
        let second = install_into(tmp.path()).expect("second");
        assert!(second.is_empty());
        assert_eq!(
            std::fs::read_to_string(skills_dir(tmp.path()).join("memory.md")).unwrap(),
            "custom\n"
        );
    }
}
