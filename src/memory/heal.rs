//! Recover a SQLite memory database after lock, stale WAL, or corruption.
//!
//! Used by `open`, `init`, `update`, `doctor` and `memory repair`. Recreating
//! the file is a last resort: the broken file is copied aside first.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode};
use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};

use super::migrations;

const OPEN_ATTEMPTS: u32 = 3;
const BUSY_SLEEP_MS: u64 = 80;

/// Outcome of opening (and possibly repairing) the memory database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealReport {
    pub path: String,
    pub opened: bool,
    pub recreated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i64>,
    pub message: String,
}

/// Open `path`, retrying locks and recreating a corrupt file after backup.
pub fn heal(path: &Path) -> NodkrayResult<(Connection, HealReport)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            NodkrayError::memory(
                "MEMORY_DB_ERROR",
                format!("could not create {}: {}", parent.display(), err),
            )
        })?;
    }

    let mut last_err: Option<NodkrayError> = None;
    let mut recreated = false;
    let mut backup: Option<String> = None;

    for attempt in 0..OPEN_ATTEMPTS {
        match try_open(path) {
            Ok(conn) => {
                let schema_version = schema_version(&conn).ok();
                return Ok((
                    conn,
                    HealReport {
                        path: path.display().to_string(),
                        opened: true,
                        recreated,
                        backup,
                        schema_version,
                        message: if recreated {
                            "recreated from backup".to_string()
                        } else {
                            "ok".to_string()
                        },
                    },
                ));
            }
            Err(err) => {
                if is_busy(&err) && attempt + 1 < OPEN_ATTEMPTS {
                    thread::sleep(Duration::from_millis(BUSY_SLEEP_MS));
                    last_err = Some(sql_err("MEMORY_DB_OPEN", &err));
                    continue;
                }
                if is_corrupt(&err) && !recreated {
                    backup = Some(quarantine(path)?);
                    last_err = Some(sql_err("MEMORY_DB_OPEN", &err));
                    recreated = true;
                    continue;
                }
                if wal_sidecars_exist(path) && attempt + 1 < OPEN_ATTEMPTS {
                    let _ = try_checkpoint(path);
                    last_err = Some(sql_err("MEMORY_DB_OPEN", &err));
                    continue;
                }
                return Err(sql_err("MEMORY_DB_OPEN", &err));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        NodkrayError::memory("MEMORY_DB_OPEN", "could not open memory database")
    }))
}

fn try_open(path: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrations::run(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=5000;
         PRAGMA synchronous=NORMAL;",
    )
}

fn schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
}

fn is_busy(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

fn is_corrupt(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    ) || err.to_string().to_ascii_lowercase().contains("not a database")
        || err.to_string().to_ascii_lowercase().contains("malformed")
}

fn wal_sidecars_exist(path: &Path) -> bool {
    path.with_file_name(format!(
        "{}-wal",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("memory.db")
    ))
    .exists()
        || path
            .with_file_name(format!(
                "{}-shm",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("memory.db")
            ))
            .exists()
}

fn try_checkpoint(path: &Path) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    Ok(())
}

fn quarantine(path: &Path) -> NodkrayResult<String> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = sidecar_path(path, &format!("corrupt.{stamp}"));
    if path.exists() {
        std::fs::rename(path, &backup).or_else(|_| {
            std::fs::copy(path, &backup)?;
            std::fs::remove_file(path)
        })?;
    }
    for suffix in ["-wal", "-shm"] {
        let name = format!(
            "{}{suffix}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("memory.db")
        );
        let sidecar = path.with_file_name(name);
        if sidecar.exists() {
            let dest = sidecar_path(&sidecar, &format!("corrupt.{stamp}"));
            let _ = std::fs::rename(&sidecar, dest);
        }
    }
    Ok(backup.display().to_string())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("memory.db");
    path.with_file_name(format!("{name}.{suffix}"))
}

fn sql_err(code: &str, err: &rusqlite::Error) -> NodkrayError {
    NodkrayError::memory(code, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heal_creates_a_fresh_database() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("memory.db");
        let (conn, report) = heal(&path).expect("heal");
        drop(conn);
        assert!(report.opened);
        assert!(!report.recreated);
        assert!(path.is_file());
    }

    #[test]
    fn heal_quarantines_garbage_and_recreates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("memory.db");
        std::fs::write(&path, b"this is not sqlite").expect("write garbage");

        let (conn, report) = heal(&path).expect("heal");
        drop(conn);
        assert!(report.recreated);
        assert!(report.backup.is_some());
        let backup = PathBuf::from(report.backup.expect("backup path"));
        assert!(backup.is_file());
        assert_eq!(std::fs::read(&backup).expect("read backup"), b"this is not sqlite");

        let again = heal(&path).expect("second heal");
        assert!(again.1.opened);
        assert!(!again.1.recreated);
    }
}
