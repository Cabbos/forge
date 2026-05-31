use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};

use super::ContinuityEvent;

pub struct ContinuityStore {
    conn: Mutex<Connection>,
}

impl ContinuityStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        ensure_parent_dir(path.as_ref())?;
        let conn = Connection::open(path.as_ref())
            .map_err(|err| format!("Failed to open continuity database: {err}"))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn record_event(&self, project_path: &str, event: &ContinuityEvent) -> Result<(), String> {
        let timestamp_ms = to_i64_timestamp(event.timestamp_ms())?;
        let event_json = serde_json::to_string(event)
            .map_err(|err| format!("Failed to serialize continuity event: {err}"))?;

        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        conn.execute(
            "INSERT INTO continuity_events
                (project_path, session_id, event_type, timestamp_ms, event_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                project_path,
                event.session_id(),
                event.event_type(),
                timestamp_ms,
                event_json
            ],
        )
        .map_err(|err| format!("Failed to record continuity event: {err}"))?;
        Ok(())
    }

    pub fn list_events_for_session(
        &self,
        project_path: &str,
        session_id: &str,
    ) -> Result<Vec<ContinuityEvent>, String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT event_json
                 FROM continuity_events
                 WHERE project_path = ?1 AND session_id = ?2
                 ORDER BY timestamp_ms ASC, id ASC",
            )
            .map_err(|err| format!("Failed to prepare continuity event query: {err}"))?;

        let rows = stmt
            .query_map(params![project_path, session_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| format!("Failed to query continuity events: {err}"))?;

        let mut events = Vec::new();
        for row in rows {
            let event_json =
                row.map_err(|err| format!("Failed to read continuity event row: {err}"))?;
            let event = serde_json::from_str(&event_json)
                .map_err(|err| format!("Failed to deserialize continuity event: {err}"))?;
            events.push(event);
        }
        Ok(events)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS continuity_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                event_json TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_continuity_events_session
                ON continuity_events(project_path, session_id, timestamp_ms, id);
            CREATE INDEX IF NOT EXISTS idx_continuity_events_type
                ON continuity_events(project_path, event_type, timestamp_ms);
            ",
        )
        .map_err(|err| format!("Failed to migrate continuity database: {err}"))?;
        Ok(())
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let parent = path.parent().map(PathBuf::from);
    if let Some(parent) = parent {
        std::fs::create_dir_all(&parent).map_err(|err| {
            format!(
                "Failed to create continuity database directory {}: {err}",
                parent.display()
            )
        })?;
    }
    Ok(())
}

fn to_i64_timestamp(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("Continuity timestamp is too large: {value}"))
}
