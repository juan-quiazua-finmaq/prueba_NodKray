//! SQLite implementation of [`MemoryRepository`] and [`TaskRepository`].
//!
//! WAL mode, foreign keys and a busy timeout are enabled on open; migrations
//! run before the connection is handed out.

use std::path::Path;

use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::{
    migrations, new_id, now_iso, search, Decision, Memory, MemoryPreview, MemoryRepository,
    NewDecision, NewMemory, NewObservation, NewReview, NewReviewCheck, NewSession, NewWorker,
    Observation, Project, Review, ReviewCheck, Session, TimelineEntry, Worker,
};
use crate::core::task::{
    NewTask, Task, TaskEvent, TaskRepository, TaskStatus, TaskSummary,
};
use crate::error::{NodkrayError, NodkrayResult};

fn sql_err(code: &str, err: rusqlite::Error) -> NodkrayError {
    NodkrayError::memory(code, err.to_string())
}

/// Aggregate counts for `nodkray project inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectStats {
    pub memories: i64,
    pub tasks: i64,
    pub decisions: i64,
    pub observations: i64,
    pub sessions: i64,
    pub reviews: i64,
}

/// A persisted project rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectRule {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub content_hash: Option<String>,
}

/// SQLite-backed repository.
pub struct SqliteMemoryRepository {
    conn: Connection,
}

impl SqliteMemoryRepository {
    /// Open (creating if needed) the database at `path` and migrate it.
    pub fn open(path: &Path) -> NodkrayResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                NodkrayError::memory(
                    "MEMORY_DB_ERROR",
                    format!("could not create {}: {}", parent.display(), err),
                )
            })?;
        }
        let mut conn = Connection::open(path).map_err(|err| sql_err("MEMORY_DB_OPEN", err))?;
        Self::configure(&conn)?;
        migrations::run(&mut conn).map_err(|err| sql_err("MEMORY_MIGRATION_FAILED", err))?;
        Ok(Self { conn })
    }

    /// In-memory database, used by tests.
    pub fn open_in_memory() -> NodkrayResult<Self> {
        let mut conn =
            Connection::open_in_memory().map_err(|err| sql_err("MEMORY_DB_OPEN", err))?;
        Self::configure(&conn)?;
        migrations::run(&mut conn).map_err(|err| sql_err("MEMORY_MIGRATION_FAILED", err))?;
        Ok(Self { conn })
    }

    fn configure(conn: &Connection) -> NodkrayResult<()> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;",
        )
        .map_err(|err| sql_err("MEMORY_PRAGMA_FAILED", err))
    }

    /// Versions recorded in `schema_migrations`, ascending.
    pub fn applied_migrations(&self) -> NodkrayResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    /// Highest applied migration version.
    pub fn schema_version(&self) -> NodkrayResult<i64> {
        Ok(self.applied_migrations()?.into_iter().max().unwrap_or(0))
    }

    /// Aggregate counts per project.
    pub fn project_stats(&self, project_id: &str) -> NodkrayResult<ProjectStats> {
        let count = |table: &str| -> NodkrayResult<i64> {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1");
            self.conn
                .query_row(&sql, params![project_id], |row| row.get(0))
                .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
        };
        Ok(ProjectStats {
            memories: count("memories")?,
            tasks: count("tasks")?,
            decisions: count("decisions")?,
            observations: count("observations")?,
            sessions: count("sessions")?,
            reviews: self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM reviews r
                     JOIN tasks t ON t.id = r.task_id
                     WHERE t.project_id = ?1",
                    params![project_id],
                    |row| row.get(0),
                )
                .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?,
        })
    }

    /// Insert or refresh a detected project rule (idempotent by path).
    pub fn upsert_project_rule(
        &self,
        project_id: &str,
        path: &str,
        kind: &str,
        content_hash: Option<&str>,
    ) -> NodkrayResult<()> {
        self.conn
            .execute(
                "INSERT INTO project_rules(id, project_id, path, kind, content_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(project_id, path) DO UPDATE SET
                     kind = excluded.kind,
                     content_hash = excluded.content_hash",
                params![new_id("rule"), project_id, path, kind, content_hash, now_iso()],
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        Ok(())
    }

    /// Project rules recorded so far, ordered by path.
    pub fn list_project_rules(&self, project_id: &str) -> NodkrayResult<Vec<ProjectRule>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, path, kind, content_hash FROM project_rules
                 WHERE project_id = ?1 ORDER BY path ASC",
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                Ok(ProjectRule {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    kind: row.get(2)?,
                    content_hash: row.get(3)?,
                })
            })
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }
}

impl SqliteMemoryRepository {
    /// Create a session together with its logical config snapshot (spec §50, §164).
    pub fn create_session(&self, new: &NewSession) -> NodkrayResult<Session> {
        let id = new_id("session");
        let now = now_iso();
        self.conn
            .execute(
                "INSERT INTO sessions(id, project_id, frontier_agent, workflow, started_at, ended_at, status, config_snapshot)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
                params![
                    id,
                    new.project_id,
                    new.frontier_agent,
                    new.workflow,
                    now,
                    new.status,
                    new.config_snapshot
                ],
            )
            .map_err(|err| sql_err("SESSION_CREATE_FAILED", err))?;
        self.session(&id)?.ok_or_else(|| {
            NodkrayError::memory("SESSION_CREATE_FAILED", "session could not be read back")
        })
    }

    /// Fetch a session by id.
    pub fn session(&self, session_id: &str) -> NodkrayResult<Option<Session>> {
        self.conn
            .query_row(
                "SELECT id, project_id, frontier_agent, workflow, started_at, ended_at, status, config_snapshot
                 FROM sessions WHERE id = ?1",
                params![session_id],
                session_from_row,
            )
            .optional()
            .map_err(|err| sql_err("SESSION_QUERY_FAILED", err))
    }

    /// Close a session with a terminal status.
    pub fn end_session(&self, session_id: &str, status: &str) -> NodkrayResult<()> {
        self.conn
            .execute(
                "UPDATE sessions SET ended_at = ?2, status = ?3 WHERE id = ?1",
                params![session_id, now_iso(), status],
            )
            .map_err(|err| sql_err("SESSION_UPDATE_FAILED", err))?;
        Ok(())
    }

    /// Create a worker row.
    pub fn create_worker(&self, new: &NewWorker) -> NodkrayResult<Worker> {
        let id = new_id("worker");
        let now = now_iso();
        self.conn
            .execute(
                "INSERT INTO workers(id, task_id, role, agent, worktree_path, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    id,
                    new.task_id,
                    new.role,
                    new.agent,
                    new.worktree_path,
                    new.status,
                    now
                ],
            )
            .map_err(|err| sql_err("WORKER_CREATE_FAILED", err))?;
        self.worker(&id)?.ok_or_else(|| {
            NodkrayError::memory("WORKER_CREATE_FAILED", "worker could not be read back")
        })
    }

    /// Fetch a worker by id.
    pub fn worker(&self, worker_id: &str) -> NodkrayResult<Option<Worker>> {
        self.conn
            .query_row(
                "SELECT id, task_id, role, agent, worktree_path, status, created_at, updated_at
                 FROM workers WHERE id = ?1",
                params![worker_id],
                worker_from_row,
            )
            .optional()
            .map_err(|err| sql_err("WORKER_QUERY_FAILED", err))
    }

    /// Update a worker status and timestamp.
    pub fn update_worker_status(&self, worker_id: &str, status: &str) -> NodkrayResult<()> {
        self.conn
            .execute(
                "UPDATE workers SET status = ?2, updated_at = ?3 WHERE id = ?1",
                params![worker_id, status, now_iso()],
            )
            .map_err(|err| sql_err("WORKER_UPDATE_FAILED", err))?;
        Ok(())
    }

    /// Record the classification result (workflow + effort) on a task.
    pub fn set_task_classification(
        &self,
        task_id: &str,
        workflow: &str,
        effort: i64,
    ) -> NodkrayResult<()> {
        self.conn
            .execute(
                "UPDATE tasks SET workflow = ?2, effort = ?3, updated_at = ?4 WHERE id = ?1",
                params![task_id, workflow, effort, now_iso()],
            )
            .map_err(|err| sql_err("TASK_UPDATE_FAILED", err))?;
        Ok(())
    }

    /// Create a review row.
    pub fn create_review(&self, new: &NewReview) -> NodkrayResult<Review> {
        let id = new_id("review");
        self.conn
            .execute(
                "INSERT INTO reviews(id, task_id, depth, status, score, verdict_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    new.task_id,
                    new.depth,
                    new.status,
                    new.score,
                    new.verdict_json,
                    now_iso()
                ],
            )
            .map_err(|err| sql_err("REVIEW_CREATE_FAILED", err))?;
        self.review(&id)?.ok_or_else(|| {
            NodkrayError::memory("REVIEW_CREATE_FAILED", "review could not be read back")
        })
    }

    /// Fetch a review by id.
    pub fn review(&self, review_id: &str) -> NodkrayResult<Option<Review>> {
        self.conn
            .query_row(
                "SELECT id, task_id, depth, status, score, verdict_json, created_at
                 FROM reviews WHERE id = ?1",
                params![review_id],
                review_from_row,
            )
            .optional()
            .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))
    }

    /// Append a review check.
    pub fn add_review_check(&self, new: &NewReviewCheck) -> NodkrayResult<ReviewCheck> {
        let id = new_id("check");
        self.conn
            .execute(
                "INSERT INTO review_checks(id, review_id, check_id, status, detail_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, new.review_id, new.check_id, new.status, new.detail_json],
            )
            .map_err(|err| sql_err("REVIEW_CHECK_FAILED", err))?;
        self.conn
            .query_row(
                "SELECT id, review_id, check_id, status, detail_json FROM review_checks WHERE id = ?1",
                params![id],
                review_check_from_row,
            )
            .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))
    }

    /// Workers, optionally scoped to a project via their parent task.
    pub fn list_workers(&self, project_id: Option<&str>) -> NodkrayResult<Vec<Worker>> {
        let sql = if project_id.is_some() {
            "SELECT w.id, w.task_id, w.role, w.agent, w.worktree_path, w.status, w.created_at, w.updated_at
             FROM workers w INNER JOIN tasks t ON t.id = w.task_id
             WHERE t.project_id = ?1
             ORDER BY w.updated_at DESC, w.id DESC"
        } else {
            "SELECT id, task_id, role, agent, worktree_path, status, created_at, updated_at
             FROM workers ORDER BY updated_at DESC, id DESC"
        };
        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| sql_err("WORKER_QUERY_FAILED", err))?;
        let rows = match project_id {
            Some(project) => stmt
                .query_map(params![project], worker_from_row)
                .map_err(|err| sql_err("WORKER_QUERY_FAILED", err))?,
            None => stmt
                .query_map([], worker_from_row)
                .map_err(|err| sql_err("WORKER_QUERY_FAILED", err))?,
        };
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("WORKER_QUERY_FAILED", err))
    }

    /// Reviews, optionally scoped to a project via their parent task.
    pub fn list_reviews(&self, project_id: Option<&str>) -> NodkrayResult<Vec<Review>> {
        let sql = if project_id.is_some() {
            "SELECT r.id, r.task_id, r.depth, r.status, r.score, r.verdict_json, r.created_at
             FROM reviews r INNER JOIN tasks t ON t.id = r.task_id
             WHERE t.project_id = ?1
             ORDER BY r.created_at DESC, r.id DESC"
        } else {
            "SELECT id, task_id, depth, status, score, verdict_json, created_at
             FROM reviews ORDER BY created_at DESC, id DESC"
        };
        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))?;
        let rows = match project_id {
            Some(project) => stmt
                .query_map(params![project], review_from_row)
                .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))?,
            None => stmt
                .query_map([], review_from_row)
                .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))?,
        };
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("REVIEW_QUERY_FAILED", err))
    }

    /// Recent task events across the database (control API / SSE).
    pub fn list_recent_events(&self, limit: usize) -> NodkrayResult<Vec<crate::core::task::TaskEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, task_id, event, payload_json, created_at FROM task_events
                 ORDER BY created_at DESC, id DESC LIMIT ?1",
            )
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let payload_json: Option<String> = row.get(3)?;
                Ok(crate::core::task::TaskEvent {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    event: row.get(2)?,
                    payload: payload_json.and_then(|text| serde_json::from_str(&text).ok()),
                    created_at: row.get(4)?,
                })
            })
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))
    }

    /// PID of the most recent `worker.started` event, if recorded.
    pub fn latest_worker_pid(&self, task_id: &str) -> NodkrayResult<Option<i32>> {
        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload_json FROM task_events
                 WHERE task_id = ?1 AND event = 'worker.started'
                 ORDER BY created_at DESC, id DESC LIMIT 1",
                params![task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;
        Ok(payload
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| value.get("pid").and_then(|pid| pid.as_i64()))
            .map(|pid| pid as i32))
    }
}

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        frontier_agent: row.get(2)?,
        workflow: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        status: row.get(6)?,
        config_snapshot: row.get(7)?,
    })
}

fn worker_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Worker> {
    Ok(Worker {
        id: row.get(0)?,
        task_id: row.get(1)?,
        role: row.get(2)?,
        agent: row.get(3)?,
        worktree_path: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn review_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Review> {
    Ok(Review {
        id: row.get(0)?,
        task_id: row.get(1)?,
        depth: row.get(2)?,
        status: row.get(3)?,
        score: row.get(4)?,
        verdict_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn review_check_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewCheck> {
    Ok(ReviewCheck {
        id: row.get(0)?,
        review_id: row.get(1)?,
        check_id: row.get(2)?,
        status: row.get(3)?,
        detail_json: row.get(4)?,
    })
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        git_remote: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn memory_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: row.get(0)?,
        project_id: row.get(1)?,
        memory_type: row.get(2)?,
        title: row.get(3)?,
        content: row.get(4)?,
        source: row.get(5)?,
        importance: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn decision_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Decision> {
    Ok(Decision {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        question: row.get(3)?,
        decision: row.get(4)?,
        rationale: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn observation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Observation> {
    Ok(Observation {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        task_id: row.get(3)?,
        observation_type: row.get(4)?,
        title: row.get(5)?,
        content: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(9)?;
    Ok(Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        parent_task_id: row.get(3)?,
        title: row.get(4)?,
        description: row.get(5)?,
        workflow: row.get(6)?,
        effort: row.get(7)?,
        role: row.get(8)?,
        status: TaskStatus::parse(&status),
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const TASK_COLUMNS: &str =
    "id, project_id, session_id, parent_task_id, title, description, workflow, effort, role, status, created_at, updated_at";

impl MemoryRepository for SqliteMemoryRepository {
    fn project(&self, root_path: &str) -> NodkrayResult<Option<Project>> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, git_remote, created_at, updated_at
                 FROM projects WHERE root_path = ?1",
                params![root_path],
                project_from_row,
            )
            .optional()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    fn project_by_id(&self, project_id: &str) -> NodkrayResult<Option<Project>> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, git_remote, created_at, updated_at
                 FROM projects WHERE id = ?1",
                params![project_id],
                project_from_row,
            )
            .optional()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    fn get_or_create_project(&self, root_path: &str) -> NodkrayResult<Project> {
        if let Some(existing) = self.project(root_path)? {
            return Ok(existing);
        }

        let name = Path::new(root_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_string());
        let now = now_iso();
        self.conn
            .execute(
                "INSERT INTO projects(id, name, root_path, git_remote, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
                params![new_id("project"), name, root_path, now],
            )
            .map_err(|err| sql_err("MEMORY_PROJECT_CREATE_FAILED", err))?;

        self.project(root_path)?.ok_or_else(|| {
            NodkrayError::memory(
                "MEMORY_PROJECT_CREATE_FAILED",
                format!("project for {root_path} could not be read back"),
            )
        })
    }

    fn set_project_git_remote(&self, project_id: &str, remote: &str) -> NodkrayResult<Project> {
        self.conn
            .execute(
                "UPDATE projects SET git_remote = ?2, updated_at = ?3 WHERE id = ?1",
                params![project_id, remote, now_iso()],
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        self.project_by_id(project_id)?.ok_or_else(|| {
            NodkrayError::memory("MEMORY_PROJECT_NOT_FOUND", format!("unknown project {project_id}"))
        })
    }

    fn save_memory(&self, new: &NewMemory) -> NodkrayResult<Memory> {
        let id = new_id("memory");
        let now = now_iso();
        self.conn
            .execute(
                "INSERT INTO memories(id, project_id, type, title, content, source, importance, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    id,
                    new.project_id,
                    new.memory_type,
                    new.title,
                    new.content,
                    new.source,
                    new.importance,
                    now
                ],
            )
            .map_err(|err| sql_err("MEMORY_SAVE_FAILED", err))?;

        self.get_memory(&id)?.ok_or_else(|| {
            NodkrayError::memory("MEMORY_SAVE_FAILED", "memory could not be read back")
        })
    }

    fn get_memory(&self, id: &str) -> NodkrayResult<Option<Memory>> {
        self.conn
            .query_row(
                "SELECT id, project_id, type, title, content, source, importance, created_at, updated_at
                 FROM memories WHERE id = ?1",
                params![id],
                memory_from_row,
            )
            .optional()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    fn search_memory(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> NodkrayResult<Vec<MemoryPreview>> {
        let Some(match_expr) = search::build_match_query(query) else {
            return Ok(Vec::new());
        };

        let (sql, mut bindings) = if project_id.is_some() {
            (
                "SELECT m.id, m.title, m.type, substr(m.content, 1, 201), bm25(memory_fts)
                 FROM memory_fts
                 JOIN memories m ON m.rowid = memory_fts.rowid
                 WHERE memory_fts MATCH ?1 AND m.project_id = ?2
                 ORDER BY bm25(memory_fts) ASC
                 LIMIT ?3 OFFSET ?4",
                vec![
                    SqlValue::Text(match_expr),
                    SqlValue::Text(project_id.unwrap_or_default().to_string()),
                ],
            )
        } else {
            (
                "SELECT m.id, m.title, m.type, substr(m.content, 1, 201), bm25(memory_fts)
                 FROM memory_fts
                 JOIN memories m ON m.rowid = memory_fts.rowid
                 WHERE memory_fts MATCH ?1
                 ORDER BY bm25(memory_fts) ASC
                 LIMIT ?2 OFFSET ?3",
                vec![SqlValue::Text(match_expr)],
            )
        };
        bindings.push(SqlValue::Integer(i64::from(limit)));
        bindings.push(SqlValue::Integer(i64::from(offset)));

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| sql_err("MEMORY_SEARCH_FAILED", err))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(bindings), |row| {
                let fragment: String = row.get(3)?;
                let rank: f64 = row.get(4)?;
                Ok(MemoryPreview {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    memory_type: row.get(2)?,
                    score: search::score_from_bm25(rank),
                    preview: search::make_preview(&fragment, search::PREVIEW_LEN),
                })
            })
            .map_err(|err| sql_err("MEMORY_SEARCH_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("MEMORY_SEARCH_FAILED", err))
    }

    fn delete_memory(&self, id: &str) -> NodkrayResult<bool> {
        let deleted = self
            .conn
            .execute("DELETE FROM memories WHERE id = ?1", params![id])
            .map_err(|err| sql_err("MEMORY_DELETE_FAILED", err))?;
        Ok(deleted > 0)
    }

    fn timeline(&self, project_id: Option<&str>, limit: u32) -> NodkrayResult<Vec<TimelineEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, 'memory' AS kind, title, created_at FROM memories
                   WHERE (?1 IS NULL OR project_id = ?1)
                 UNION ALL
                 SELECT id, 'decision', question, created_at FROM decisions
                   WHERE (?1 IS NULL OR project_id = ?1)
                 UNION ALL
                 SELECT id, 'observation', title, created_at FROM observations
                   WHERE (?1 IS NULL OR project_id = ?1)
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?2",
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        let rows = stmt
            .query_map(params![project_id, i64::from(limit)], |row| {
                Ok(TimelineEntry {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    fn save_decision(&self, new: &NewDecision) -> NodkrayResult<Decision> {
        let id = new_id("decision");
        self.conn
            .execute(
                "INSERT INTO decisions(id, project_id, task_id, question, decision, rationale, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    new.project_id,
                    new.task_id,
                    new.question,
                    new.decision,
                    new.rationale,
                    now_iso()
                ],
            )
            .map_err(|err| sql_err("MEMORY_SAVE_FAILED", err))?;
        self.conn
            .query_row(
                "SELECT id, project_id, task_id, question, decision, rationale, created_at
                 FROM decisions WHERE id = ?1",
                params![id],
                decision_from_row,
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }

    fn save_observation(&self, new: &NewObservation) -> NodkrayResult<Observation> {
        let id = new_id("observation");
        self.conn
            .execute(
                "INSERT INTO observations(id, project_id, session_id, task_id, type, title, content, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    new.project_id,
                    new.session_id,
                    new.task_id,
                    new.observation_type,
                    new.title,
                    new.content,
                    now_iso()
                ],
            )
            .map_err(|err| sql_err("MEMORY_SAVE_FAILED", err))?;
        self.conn
            .query_row(
                "SELECT id, project_id, session_id, task_id, type, title, content, created_at
                 FROM observations WHERE id = ?1",
                params![id],
                observation_from_row,
            )
            .map_err(|err| sql_err("MEMORY_QUERY_FAILED", err))
    }
}

impl TaskRepository for SqliteMemoryRepository {
    fn create_task(&self, new: &NewTask) -> NodkrayResult<Task> {
        let id = new_id("task");
        let now = now_iso();
        self.conn
            .execute(
                "INSERT INTO tasks(id, project_id, session_id, parent_task_id, title, description,
                                   workflow, effort, role, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    id,
                    new.project_id,
                    new.session_id,
                    new.parent_task_id,
                    new.title,
                    new.description,
                    new.workflow,
                    new.effort,
                    new.role,
                    TaskStatus::Pending.as_str(),
                    now
                ],
            )
            .map_err(|err| sql_err("TASK_CREATE_FAILED", err))?;

        self.conn
            .execute(
                "INSERT INTO task_events(id, task_id, event, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![new_id("event"), id, "task.created", Option::<String>::None, now],
            )
            .map_err(|err| sql_err("TASK_EVENT_FAILED", err))?;

        self.get_task(&id)?.ok_or_else(|| {
            NodkrayError::memory("TASK_CREATE_FAILED", "task could not be read back")
        })
    }

    fn get_task(&self, task_id: &str) -> NodkrayResult<Option<Task>> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
        self.conn
            .query_row(&sql, params![task_id], task_from_row)
            .optional()
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))
    }

    fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        event: &str,
        payload: Option<serde_json::Value>,
    ) -> NodkrayResult<Task> {
        let payload_json = match payload {
            Some(value) => Some(serde_json::to_string(&value).map_err(|err| {
                NodkrayError::internal("TASK_EVENT_SERIALIZE_FAILED", err.to_string())
            })?),
            None => None,
        };
        let now = now_iso();

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| sql_err("TASK_UPDATE_FAILED", err))?;

        let changed = tx
            .execute(
                "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
                params![task_id, status.as_str(), now],
            )
            .map_err(|err| sql_err("TASK_UPDATE_FAILED", err))?;

        if changed == 0 {
            // Dropping the transaction rolls it back.
            return Err(NodkrayError::user_input(
                "TASK_NOT_FOUND",
                format!("unknown task {task_id}"),
            ));
        }

        tx.execute(
            "INSERT INTO task_events(id, task_id, event, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_id("event"), task_id, event, payload_json, now],
        )
        .map_err(|err| sql_err("TASK_EVENT_FAILED", err))?;

        tx.commit().map_err(|err| sql_err("TASK_UPDATE_FAILED", err))?;

        self.get_task(task_id)?.ok_or_else(|| {
            NodkrayError::memory("TASK_NOT_FOUND", format!("unknown task {task_id}"))
        })
    }

    fn list_tasks(
        &self,
        project_id: Option<&str>,
        include_terminal: bool,
    ) -> NodkrayResult<Vec<TaskSummary>> {
        let mut sql = String::from("SELECT id, title, status, workflow, updated_at FROM tasks");
        let mut clauses: Vec<String> = Vec::new();
        if project_id.is_some() {
            clauses.push("project_id = ?1".to_string());
        }
        if !include_terminal {
            let list = crate::core::task::TERMINAL_STATES
                .iter()
                .map(|state| format!("'{state}'"))
                .collect::<Vec<_>>()
                .join(", ");
            // The terminal-state list is a compile-time constant, not user input.
            clauses.push(format!("status NOT IN ({list})"));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY updated_at DESC, id DESC");

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<TaskSummary> {
            let status: String = row.get(2)?;
            Ok(TaskSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                status: TaskStatus::parse(&status),
                workflow: row.get(3)?,
                updated_at: row.get(4)?,
            })
        };

        let rows = match project_id {
            Some(project) => stmt
                .query_map(params![project], map_row)
                .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?,
            None => stmt
                .query_map([], map_row)
                .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?,
        };
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))
    }

    fn append_event(
        &self,
        task_id: &str,
        event: &str,
        payload: Option<serde_json::Value>,
    ) -> NodkrayResult<()> {
        let payload_json = match payload {
            Some(value) => Some(serde_json::to_string(&value).map_err(|err| {
                NodkrayError::internal("TASK_EVENT_SERIALIZE_FAILED", err.to_string())
            })?),
            None => None,
        };
        self.conn
            .execute(
                "INSERT INTO task_events(id, task_id, event, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![new_id("event"), task_id, event, payload_json, now_iso()],
            )
            .map_err(|err| sql_err("TASK_EVENT_FAILED", err))?;
        Ok(())
    }

    fn list_task_events(&self, task_id: &str) -> NodkrayResult<Vec<TaskEvent>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, task_id, event, payload_json, created_at FROM task_events
                 WHERE task_id = ?1 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;
        let rows = stmt
            .query_map(params![task_id], |row| {
                let payload_json: Option<String> = row.get(3)?;
                Ok(TaskEvent {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    event: row.get(2)?,
                    payload: payload_json.and_then(|text| serde_json::from_str(&text).ok()),
                    created_at: row.get(4)?,
                })
            })
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|err| sql_err("TASK_QUERY_FAILED", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{NewMemory, TimelineEntry};

    fn repo() -> SqliteMemoryRepository {
        SqliteMemoryRepository::open_in_memory().expect("in-memory repo")
    }

    fn save(repo: &SqliteMemoryRepository, project_id: &str, title: &str, content: &str) -> Memory {
        repo.save_memory(&NewMemory {
            project_id: project_id.to_string(),
            memory_type: "decision".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            source: None,
            importance: 1,
        })
        .expect("save")
    }

    #[test]
    fn migrations_are_idempotent_across_opens() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("memory.db");

        let first = SqliteMemoryRepository::open(&path).expect("first open");
        let version_after_first = first.schema_version().expect("version");
        drop(first);

        let second = SqliteMemoryRepository::open(&path).expect("second open");
        assert_eq!(second.schema_version().expect("version"), version_after_first);
        assert_eq!(
            second.applied_migrations().expect("applied").len(),
            migrations::MIGRATIONS.len()
        );
    }

    #[test]
    fn fts_index_tracks_insert_update_and_delete() {
        let repo = repo();
        let project = repo
            .get_or_create_project("/tmp/alpha")
            .expect("project");

        let saved = save(&repo, &project.id, "alpha marker", "first content");
        assert_eq!(
            repo.search_memory("alpha", Some(&project.id), 10, 0)
                .expect("search")
                .len(),
            1
        );

        // Update via UPDATE statement so the update trigger fires.
        repo.conn
            .execute(
                "UPDATE memories SET title = ?2, content = ?3 WHERE id = ?1",
                params![saved.id, "beta marker", "second content"],
            )
            .expect("update");
        assert!(
            repo.search_memory("alpha", Some(&project.id), 10, 0)
                .expect("search")
                .is_empty(),
            "old term must leave the index"
        );
        assert_eq!(
            repo.search_memory("beta", Some(&project.id), 10, 0)
                .expect("search")
                .len(),
            1
        );

        assert!(repo.delete_memory(&saved.id).expect("delete"));
        assert!(
            repo.search_memory("beta", Some(&project.id), 10, 0)
                .expect("search")
                .is_empty(),
            "deleted rows must leave the index"
        );
    }

    #[test]
    fn search_is_isolated_per_project_and_global_crosses() {
        let repo = repo();
        let alpha = repo.get_or_create_project("/tmp/alpha").expect("alpha");
        let beta = repo.get_or_create_project("/tmp/beta").expect("beta");

        save(&repo, &alpha.id, "alphaonly knowledge", "belongs to alpha");
        save(&repo, &beta.id, "betaonly knowledge", "belongs to beta");

        assert_eq!(
            repo.search_memory("alphaonly", Some(&beta.id), 10, 0)
                .expect("beta search")
                .len(),
            0
        );
        assert_eq!(
            repo.search_memory("alphaonly", Some(&alpha.id), 10, 0)
                .expect("alpha search")
                .len(),
            1
        );
        assert_eq!(
            repo.search_memory("alphaonly", None, 10, 0)
                .expect("global search")
                .len(),
            1
        );
    }

    #[test]
    fn get_or_create_project_is_deduplicated() {
        let repo = repo();
        let first = repo.get_or_create_project("/tmp/dup").expect("first");
        let second = repo.get_or_create_project("/tmp/dup").expect("second");
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn timeline_orders_newest_first() {
        let repo = repo();
        let project = repo.get_or_create_project("/tmp/timeline").expect("project");

        repo.save_memory(&NewMemory {
            project_id: project.id.clone(),
            memory_type: "convention".to_string(),
            title: "oldest".to_string(),
            content: "oldest".to_string(),
            source: None,
            importance: 0,
        })
        .expect("save memory");
        std::thread::sleep(std::time::Duration::from_millis(3));
        repo.save_decision(&NewDecision {
            project_id: project.id.clone(),
            task_id: None,
            question: "middle".to_string(),
            decision: "yes".to_string(),
            rationale: None,
        })
        .expect("save decision");
        std::thread::sleep(std::time::Duration::from_millis(3));
        repo.save_observation(&NewObservation {
            project_id: project.id.clone(),
            session_id: None,
            task_id: None,
            observation_type: "note".to_string(),
            title: "newest".to_string(),
            content: "newest".to_string(),
        })
        .expect("save observation");

        let entries: Vec<TimelineEntry> = repo.timeline(Some(&project.id), 10).expect("timeline");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].title, "newest");
        assert_eq!(entries[0].kind, "observation");
        assert_eq!(entries[1].title, "middle");
        assert_eq!(entries[2].title, "oldest");
    }

    #[test]
    fn blank_search_returns_empty() {
        let repo = repo();
        assert!(repo.search_memory("   ", None, 10, 0).expect("search").is_empty());
    }
}
