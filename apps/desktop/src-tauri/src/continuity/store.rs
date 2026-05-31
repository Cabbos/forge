use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};

use super::{ContinuityEvent, ExperienceKind, ExperienceMemory, ExperienceStatus};

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

    pub fn upsert_experiences(
        &self,
        experiences: &[ExperienceMemory],
    ) -> Result<Vec<ExperienceMemory>, String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut inserted = Vec::new();
        for experience in experiences {
            let created_at_ms = to_i64_timestamp(experience.created_at_ms)?;
            let updated_at_ms = to_i64_timestamp(experience.updated_at_ms)?;
            let experience_json = serde_json::to_string(experience)
                .map_err(|err| format!("Failed to serialize continuity experience: {err}"))?;
            let tags_json = serde_json::to_string(&experience.tags)
                .map_err(|err| format!("Failed to serialize continuity experience tags: {err}"))?;

            let changed = conn
                .execute(
                    "INSERT OR IGNORE INTO continuity_experiences
                        (
                            id,
                            project_path,
                            source_session_id,
                            kind,
                            status,
                            title,
                            body,
                            confidence,
                            created_at_ms,
                            updated_at_ms,
                            tags_json,
                            experience_json
                        )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        experience.id,
                        experience.project_path.as_deref(),
                        experience.source_session_id.as_deref(),
                        experience_kind_key(&experience.kind),
                        experience_status_key(&experience.status),
                        experience.title,
                        experience.body,
                        experience.confidence,
                        created_at_ms,
                        updated_at_ms,
                        tags_json,
                        experience_json,
                    ],
                )
                .map_err(|err| format!("Failed to upsert continuity experience: {err}"))?;
            if changed > 0 {
                inserted.push(experience.clone());
            }
        }
        Ok(inserted)
    }

    pub fn list_experiences_for_project(
        &self,
        project_path: &str,
    ) -> Result<Vec<ExperienceMemory>, String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT experience_json
                 FROM continuity_experiences
                 WHERE project_path = ?1
                 ORDER BY created_at_ms ASC, id ASC",
            )
            .map_err(|err| format!("Failed to prepare continuity experience query: {err}"))?;

        let rows = stmt
            .query_map(params![project_path], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Failed to query continuity experiences: {err}"))?;

        let mut experiences = Vec::new();
        for row in rows {
            let experience_json =
                row.map_err(|err| format!("Failed to read continuity experience row: {err}"))?;
            let experience = serde_json::from_str(&experience_json)
                .map_err(|err| format!("Failed to deserialize continuity experience: {err}"))?;
            experiences.push(experience);
        }
        Ok(experiences)
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
            CREATE TABLE IF NOT EXISTS continuity_experiences (
                id TEXT PRIMARY KEY,
                project_path TEXT,
                source_session_id TEXT,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                tags_json TEXT NOT NULL,
                experience_json TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_continuity_experiences_project
                ON continuity_experiences(project_path, status, kind, updated_at_ms);
            CREATE INDEX IF NOT EXISTS idx_continuity_experiences_session
                ON continuity_experiences(source_session_id, created_at_ms);
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

fn experience_kind_key(kind: &ExperienceKind) -> &'static str {
    match kind {
        ExperienceKind::Lesson => "lesson",
        ExperienceKind::BugPattern => "bug_pattern",
        ExperienceKind::Workflow => "workflow",
        ExperienceKind::Decision => "decision",
        ExperienceKind::Preference => "preference",
        ExperienceKind::ProjectFact => "project_fact",
    }
}

fn experience_status_key(status: &ExperienceStatus) -> &'static str {
    match status {
        ExperienceStatus::Candidate => "candidate",
        ExperienceStatus::Accepted => "accepted",
        ExperienceStatus::Pinned => "pinned",
        ExperienceStatus::Forgotten => "forgotten",
        ExperienceStatus::Archived => "archived",
    }
}
