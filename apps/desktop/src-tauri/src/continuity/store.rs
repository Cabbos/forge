use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};

use super::{
    should_reject_experience_lesson, ContinuityEvent, ExperienceKind, ExperienceMemory,
    ExperienceStatus, ReflectionEvent,
};

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

    pub fn list_unformed_reflections_for_session(
        &self,
        project_path: &str,
        session_id: &str,
    ) -> Result<Vec<ReflectionEvent>, String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT e.event_json
                 FROM continuity_events e
                 WHERE e.project_path = ?1
                    AND e.session_id = ?2
                    AND e.event_type = 'reflection'
                    AND NOT EXISTS (
                        SELECT 1
                        FROM continuity_formed_reflections f
                        WHERE f.project_path = e.project_path
                           AND f.session_id = e.session_id
                           AND f.timestamp_ms = e.timestamp_ms
                    )
                 ORDER BY e.timestamp_ms ASC, e.id ASC",
            )
            .map_err(|err| format!("Failed to prepare unformed reflection query: {err}"))?;

        let rows = stmt
            .query_map(params![project_path, session_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| format!("Failed to query unformed reflections: {err}"))?;

        let mut reflections = Vec::new();
        for row in rows {
            let event_json =
                row.map_err(|err| format!("Failed to read unformed reflection row: {err}"))?;
            let event: ContinuityEvent = serde_json::from_str(&event_json)
                .map_err(|err| format!("Failed to deserialize unformed reflection: {err}"))?;
            if let ContinuityEvent::Reflection(reflection) = event {
                reflections.push(reflection);
            }
        }
        Ok(reflections)
    }

    pub fn mark_reflections_formed(
        &self,
        project_path: &str,
        reflections: &[ReflectionEvent],
    ) -> Result<(), String> {
        if reflections.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        for reflection in reflections {
            let timestamp_ms = to_i64_timestamp(reflection.timestamp_ms)?;
            conn.execute(
                "INSERT OR IGNORE INTO continuity_formed_reflections
                    (project_path, session_id, timestamp_ms)
                 VALUES (?1, ?2, ?3)",
                params![project_path, reflection.session_id, timestamp_ms],
            )
            .map_err(|err| format!("Failed to mark reflection as formed: {err}"))?;
        }
        Ok(())
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
            let tags_text = experience.tags.join(" ");

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
                conn.execute(
                    "INSERT OR IGNORE INTO continuity_experiences_fts
                        (id, project_path, title, body, tags)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        experience.id,
                        experience.project_path.as_deref(),
                        experience.title,
                        experience.body,
                        tags_text,
                    ],
                )
                .map_err(|err| format!("Failed to index continuity experience: {err}"))?;
                inserted.push(experience.clone());
            }
        }
        Ok(inserted)
    }

    pub fn update_experience_status(
        &self,
        project_path: &str,
        experience_id: &str,
        status: ExperienceStatus,
        now_ms: u64,
    ) -> Result<ExperienceMemory, String> {
        let updated_at_ms = to_i64_timestamp(now_ms)?;
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let experience_json = conn
            .query_row(
                "SELECT experience_json
                 FROM continuity_experiences
                 WHERE project_path = ?1 AND id = ?2",
                params![project_path, experience_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|err| format!("Failed to load continuity experience: {err}"))?;
        let mut experience: ExperienceMemory =
            serde_json::from_str(&experience_json).map_err(|err| {
                format!("Failed to deserialize continuity experience for update: {err}")
            })?;
        experience.status = status;
        experience.updated_at_ms = now_ms;
        let next_json = serde_json::to_string(&experience)
            .map_err(|err| format!("Failed to serialize continuity experience update: {err}"))?;

        conn.execute(
            "UPDATE continuity_experiences
             SET status = ?3,
                 updated_at_ms = ?4,
                 experience_json = ?5
             WHERE project_path = ?1 AND id = ?2",
            params![
                project_path,
                experience_id,
                experience_status_key(&experience.status),
                updated_at_ms,
                next_json,
            ],
        )
        .map_err(|err| format!("Failed to update continuity experience: {err}"))?;
        Ok(experience)
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

    pub fn search_experiences_for_project(
        &self,
        project_path: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExperienceMemory>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let Some(query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let limit = i64::try_from(limit)
            .map_err(|_| format!("Continuity search limit is too large: {limit}"))?;

        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT e.experience_json
                 FROM continuity_experiences_fts
                 JOIN continuity_experiences e
                    ON e.id = continuity_experiences_fts.id
                 WHERE continuity_experiences_fts.project_path = ?1
                    AND continuity_experiences_fts MATCH ?2
                    AND e.status NOT IN ('forgotten', 'archived')
                 ORDER BY
                    bm25(continuity_experiences_fts),
                    e.confidence DESC,
                    e.updated_at_ms DESC,
                    e.id ASC
                 LIMIT ?3",
            )
            .map_err(|err| format!("Failed to prepare continuity experience search: {err}"))?;

        let rows = stmt
            .query_map(params![project_path, query, limit], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| format!("Failed to search continuity experiences: {err}"))?;

        let mut experiences = Vec::new();
        for row in rows {
            let experience_json =
                row.map_err(|err| format!("Failed to read continuity search row: {err}"))?;
            let experience = serde_json::from_str(&experience_json).map_err(|err| {
                format!("Failed to deserialize continuity search experience: {err}")
            })?;
            experiences.push(experience);
        }
        Ok(experiences)
    }

    pub fn recall_experiences_for_project(
        &self,
        project_path: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExperienceMemory>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let Some(query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let limit = i64::try_from(limit)
            .map_err(|_| format!("Continuity recall limit is too large: {limit}"))?;

        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT e.experience_json
                 FROM continuity_experiences_fts
                 JOIN continuity_experiences e
                    ON e.id = continuity_experiences_fts.id
                 WHERE continuity_experiences_fts.project_path = ?1
                    AND continuity_experiences_fts MATCH ?2
                    AND e.status IN ('accepted', 'pinned')
                 ORDER BY
                    CASE e.status WHEN 'pinned' THEN 0 ELSE 1 END,
                    bm25(continuity_experiences_fts),
                    e.confidence DESC,
                    e.updated_at_ms DESC,
                    e.id ASC
                 LIMIT ?3",
            )
            .map_err(|err| format!("Failed to prepare continuity experience recall: {err}"))?;

        let rows = stmt
            .query_map(params![project_path, query, limit], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| format!("Failed to recall continuity experiences: {err}"))?;

        let mut experiences = Vec::new();
        for row in rows {
            let experience_json =
                row.map_err(|err| format!("Failed to read continuity recall row: {err}"))?;
            let experience = serde_json::from_str(&experience_json).map_err(|err| {
                format!("Failed to deserialize continuity recall experience: {err}")
            })?;
            experiences.push(experience);
        }
        Ok(experiences)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|err| err.into_inner());
        let formed_reflections_table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(1)
                 FROM sqlite_master
                 WHERE type = 'table' AND name = 'continuity_formed_reflections'",
                [],
                |row| row.get(0),
            )
            .map_err(|err| format!("Failed to inspect continuity migrations: {err}"))?;
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
            CREATE TABLE IF NOT EXISTS continuity_formed_reflections (
                project_path TEXT NOT NULL,
                session_id TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                created_at TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (project_path, session_id, timestamp_ms)
            );
            CREATE INDEX IF NOT EXISTS idx_continuity_formed_reflections_session
                ON continuity_formed_reflections(project_path, session_id, timestamp_ms);
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
            CREATE VIRTUAL TABLE IF NOT EXISTS continuity_experiences_fts
                USING fts5(
                    id UNINDEXED,
                    project_path UNINDEXED,
                    title,
                    body,
                    tags,
                    tokenize = 'unicode61'
                );
            ",
        )
        .map_err(|err| format!("Failed to migrate continuity database: {err}"))?;
        if formed_reflections_table_exists == 0 {
            backfill_formed_reflections(&conn)?;
        }
        prune_low_quality_candidate_experiences(&conn)?;
        rebuild_experience_fts(&conn)?;
        Ok(())
    }
}

fn backfill_formed_reflections(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO continuity_formed_reflections
            (project_path, session_id, timestamp_ms)
         SELECT project_path, session_id, timestamp_ms
         FROM continuity_events
         WHERE event_type = 'reflection'",
        [],
    )
    .map_err(|err| format!("Failed to backfill formed reflections: {err}"))?;
    Ok(())
}

fn prune_low_quality_candidate_experiences(conn: &Connection) -> Result<(), String> {
    let ids = {
        let mut stmt = conn
            .prepare(
                "SELECT id, body
                 FROM continuity_experiences
                 WHERE status = 'candidate' AND kind = 'lesson'",
            )
            .map_err(|err| format!("Failed to prepare continuity experience cleanup: {err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| format!("Failed to query continuity experience cleanup: {err}"))?;

        let mut ids = Vec::new();
        for row in rows {
            let (id, body) =
                row.map_err(|err| format!("Failed to read continuity cleanup row: {err}"))?;
            if should_reject_experience_lesson(&body) {
                ids.push(id);
            }
        }
        ids
    };

    for id in ids {
        conn.execute(
            "DELETE FROM continuity_experiences WHERE id = ?1",
            params![id],
        )
        .map_err(|err| format!("Failed to prune continuity experience: {err}"))?;
        conn.execute(
            "DELETE FROM continuity_experiences_fts WHERE id = ?1",
            params![id],
        )
        .map_err(|err| format!("Failed to prune continuity experience index: {err}"))?;
    }
    Ok(())
}

fn rebuild_experience_fts(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM continuity_experiences_fts", [])
        .map_err(|err| format!("Failed to clear continuity experience index: {err}"))?;
    conn.execute(
        "INSERT INTO continuity_experiences_fts
            (id, project_path, title, body, tags)
         SELECT id, project_path, title, body, tags_json
         FROM continuity_experiences",
        [],
    )
    .map_err(|err| format!("Failed to rebuild continuity experience index: {err}"))?;
    Ok(())
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

fn fts_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|ch: char| !ch.is_alphanumeric())
                .replace('"', "\"\"")
                .to_lowercase()
        })
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>();

    (!terms.is_empty()).then(|| terms.join(" "))
}
