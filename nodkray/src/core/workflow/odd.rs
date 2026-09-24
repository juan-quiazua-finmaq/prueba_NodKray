//! ODD — Organic Driven Development (spec §22).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NodkrayError, NodkrayResult};

/// Minimum ODD artefact content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OddTask {
    pub objective: String,
    pub context: String,
    pub scope: String,
    pub constraints: String,
    pub expected_result: String,
    pub validation: String,
}

impl OddTask {
    pub fn from_description(description: &str) -> Self {
        Self {
            objective: description.trim().to_string(),
            context: String::new(),
            scope: String::new(),
            constraints: String::new(),
            expected_result: String::new(),
            validation: String::new(),
        }
    }

    pub fn render(&self) -> String {
        format!(
            "# Task\n\n## Objective\n\n{}\n\n## Context\n\n{}\n\n## Scope\n\n{}\n\n## Constraints\n\n{}\n\n## Expected Result\n\n{}\n\n## Validation\n\n{}\n",
            empty_as_placeholder(&self.objective),
            empty_as_placeholder(&self.context),
            empty_as_placeholder(&self.scope),
            empty_as_placeholder(&self.constraints),
            empty_as_placeholder(&self.expected_result),
            empty_as_placeholder(&self.validation),
        )
    }
}

fn empty_as_placeholder(value: &str) -> &str {
    if value.trim().is_empty() {
        "_To be filled._"
    } else {
        value.trim()
    }
}

/// Directory for a task artefact: `.nodkray/tasks/<task-id>/`.
pub fn task_dir(project_root: &Path, task_id: &str) -> PathBuf {
    project_root.join(".nodkray").join("tasks").join(task_id)
}

/// Write `.nodkray/tasks/<task-id>/task.md`. Existing files are not overwritten.
pub fn write_task_md(
    project_root: &Path,
    task_id: &str,
    task: &OddTask,
) -> NodkrayResult<PathBuf> {
    if task_id.trim().is_empty() {
        return Err(NodkrayError::user_input(
            "INVALID_TASK_ID",
            "ODD task id must not be empty",
        ));
    }
    let dir = task_dir(project_root, task_id);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("task.md");
    if path.exists() {
        return Ok(path);
    }
    std::fs::write(&path, task.render())?;
    Ok(path)
}

/// Load an existing ODD artefact.
pub fn read_task_md(project_root: &Path, task_id: &str) -> NodkrayResult<String> {
    let path = task_dir(project_root, task_id).join("task.md");
    std::fs::read_to_string(&path).map_err(|err| {
        NodkrayError::user_input(
            "ODD_TASK_NOT_FOUND",
            format!("{}: {err}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_minimum_sections() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_task_md(
            tmp.path(),
            "task_01",
            &OddTask::from_description("Add JWT auth"),
        )
        .expect("write");
        let text = std::fs::read_to_string(path).expect("read");
        for heading in [
            "# Task",
            "## Objective",
            "## Context",
            "## Scope",
            "## Constraints",
            "## Expected Result",
            "## Validation",
        ] {
            assert!(text.contains(heading), "missing {heading}");
        }
        assert!(text.contains("Add JWT auth"));
    }

    #[test]
    fn does_not_overwrite_existing_task_md() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = write_task_md(
            tmp.path(),
            "task_01",
            &OddTask::from_description("first"),
        )
        .expect("first");
        std::fs::write(&first, "kept\n").expect("overwrite");
        write_task_md(
            tmp.path(),
            "task_01",
            &OddTask::from_description("second"),
        )
        .expect("second");
        assert_eq!(std::fs::read_to_string(&first).expect("read"), "kept\n");
    }
}
