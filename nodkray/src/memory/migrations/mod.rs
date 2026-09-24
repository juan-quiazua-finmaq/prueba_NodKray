//! Versioned, embedded schema migrations (spec §48-§55, §111).
//!
//! Migrations are Rust constants, not files loaded from the repository at
//! runtime, so a shipped binary is self-contained. Applied versions are traced
//! in `schema_migrations`; the runner applies only missing migrations, each in
//! its own transaction, and is therefore idempotent.

use rusqlite::Connection;

/// A single forward migration.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}

/// Ordered list of migrations.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "initial schema: projects, sessions, tasks, events, memory, workers, reviews, rules, FTS5",
        sql: M001_INITIAL,
    },
    Migration {
        version: 2,
        description: "sessions: store the logical config snapshot (§164)",
        sql: M002_SESSION_SNAPSHOT,
    },
];

/// Migration 002 — config snapshot column on sessions (spec §164).
const M002_SESSION_SNAPSHOT: &str = r#"
ALTER TABLE sessions ADD COLUMN config_snapshot TEXT;
"#;

/// Migration 001 — the full V1 schema.
const M001_INITIAL: &str = r#"
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    git_remote TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    frontier_agent TEXT,
    workflow TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT,
    parent_task_id TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    workflow TEXT NOT NULL,
    effort INTEGER,
    role TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE task_events (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    event TEXT NOT NULL,
    payload_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id)
);

CREATE TABLE observations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT,
    task_id TEXT,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT,
    question TEXT NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT,
    importance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE workers (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    role TEXT NOT NULL,
    agent TEXT NOT NULL,
    worktree_path TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id)
);

CREATE TABLE reviews (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    depth TEXT NOT NULL,
    status TEXT NOT NULL,
    score REAL,
    verdict_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id)
);

CREATE TABLE review_checks (
    id TEXT PRIMARY KEY,
    review_id TEXT NOT NULL,
    check_id TEXT NOT NULL,
    status TEXT NOT NULL,
    detail_json TEXT,
    FOREIGN KEY(review_id) REFERENCES reviews(id)
);

CREATE TABLE project_rules (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    path TEXT NOT NULL,
    kind TEXT NOT NULL,
    content_hash TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id),
    UNIQUE(project_id, path)
);

-- FTS5 external-content index over memories (spec §55).
CREATE VIRTUAL TABLE memory_fts USING fts5(
    title,
    content,
    content='memories',
    content_rowid='rowid'
);

CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memory_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO memory_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
END;

CREATE INDEX idx_sessions_project ON sessions(project_id);
CREATE INDEX idx_tasks_project ON tasks(project_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_task_events_task ON task_events(task_id);
CREATE INDEX idx_observations_project ON observations(project_id);
CREATE INDEX idx_decisions_project ON decisions(project_id);
CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_workers_task ON workers(task_id);
CREATE INDEX idx_reviews_task ON reviews(task_id);
CREATE INDEX idx_review_checks_review ON review_checks(review_id);
CREATE INDEX idx_project_rules_project ON project_rules(project_id);
"#;

/// Latest known schema version.
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

/// Apply all missing migrations. Idempotent and transactional per migration.
pub fn run(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;

    let mut current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    for migration in MIGRATIONS {
        if migration.version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations(version, description, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                migration.version,
                migration.description,
                crate::memory::now_iso()
            ],
        )?;
        tx.commit()?;
        current = migration.version;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_is_idempotent() {
        let mut conn = Connection::open_in_memory().expect("open");
        run(&mut conn).expect("first run");
        run(&mut conn).expect("second run");

        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .expect("count");
        assert_eq!(applied, MIGRATIONS.len() as i64);
        assert!(latest_version() >= 2);
    }

    #[test]
    fn creates_all_expected_tables() {
        let mut conn = Connection::open_in_memory().expect("open");
        run(&mut conn).expect("migrate");
        for table in [
            "projects",
            "sessions",
            "tasks",
            "task_events",
            "observations",
            "decisions",
            "memories",
            "workers",
            "reviews",
            "review_checks",
            "project_rules",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![table],
                    |row| row.get(0),
                )
                .expect("query table");
            assert_eq!(count, 1, "missing table {table}");
        }
    }
}
