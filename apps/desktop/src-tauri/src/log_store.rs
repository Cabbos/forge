//! Structured event log store — append-only JSON-lines log with rotation.
//!
//! Each entry is one line of JSON.  The store rotates files when they exceed
//! `MAX_LOG_BYTES` (5 MiB), keeping up to `MAX_ROTATIONS` (3) old files.

use crate::redaction::{global_redactor, PersistentLogRedactor};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB
const MAX_ROTATIONS: u32 = 3;

/// A single structured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unix epoch milliseconds.
    pub timestamp_ms: u64,
    /// "INFO" | "WARN" | "ERROR" | "PANIC"
    pub level: String,
    /// Source module / subsystem (e.g. "gateway", "session", "agent").
    pub source: String,
    /// Human-readable message.
    pub message: String,
    /// Optional session id if the entry relates to a specific session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Append-only structured log store with rotation.
pub struct LogStore {
    path: PathBuf,
    redactor: Arc<PersistentLogRedactor>,
    write_mutex: Mutex<()>,
}

impl LogStore {
    /// Default path: `~/.forge/logs/forge.log`
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".forge")
            .join("logs")
            .join("forge.log")
    }

    pub fn new(path: PathBuf) -> Self {
        Self::new_with_redactor(path, global_redactor())
    }

    pub(crate) fn new_with_redactor(path: PathBuf, redactor: Arc<PersistentLogRedactor>) -> Self {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        Self {
            path,
            redactor,
            write_mutex: Mutex::new(()),
        }
    }

    /// Append a log entry as one JSON line.
    pub fn append(&self, entry: &LogEntry) -> Result<(), String> {
        let value = serde_json::to_value(entry).map_err(|_| "serialize log entry".to_string())?;
        let redacted = self
            .redactor
            .redact_json(&value)
            .map_err(|_| "redaction failed".to_string())?;
        let entry: LogEntry =
            serde_json::from_value(redacted).map_err(|_| "serialize log entry".to_string())?;
        let mut json =
            serde_json::to_string(&entry).map_err(|_| "serialize log entry".to_string())?;
        json.push('\n');

        let _guard = self.write_mutex.lock().unwrap_or_else(|e| e.into_inner());

        // Rotate if needed.
        if let Ok(meta) = fs::metadata(&self.path) {
            if meta.len() >= MAX_LOG_BYTES {
                self.rotate()?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open log: {e}"))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("write log: {e}"))?;

        Ok(())
    }

    /// Read the most recent `limit` log entries, optionally filtered by level.
    pub fn read_recent(
        &self,
        limit: usize,
        level_filter: Option<&str>,
    ) -> Result<Vec<LogEntry>, String> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("open log: {e}")),
        };

        let reader = BufReader::new(file);
        let mut entries: Vec<LogEntry> = Vec::new();

        for line in reader.lines() {
            match line {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                        if let Some(filter) = level_filter {
                            if entry.level.to_uppercase() != filter.to_uppercase() {
                                continue;
                            }
                        }
                        entries.push(entry);
                    }
                }
                Err(_) => continue,
            }
        }

        // Return the tail.
        if entries.len() > limit {
            entries = entries.split_off(entries.len() - limit);
        }

        Ok(entries)
    }

    // ── rotation ──────────────────────────────────────────────────────────

    fn rotate(&self) -> Result<(), String> {
        // Shift: .2 → .3, .1 → .2, current → .1
        for i in (1..=MAX_ROTATIONS).rev() {
            let old = if i == 1 {
                self.path.clone()
            } else {
                self.path.with_extension(format!("log.{}.gz", i - 1))
            };
            let new = self.path.with_extension(format!("log.{}.gz", i));

            if old.exists() && i == MAX_ROTATIONS {
                let _ = fs::remove_file(&new);
            }
            if old.exists() {
                let _ = fs::rename(&old, &new);
            }
        }

        // Create a fresh empty log file.
        File::create(&self.path).map_err(|e| format!("create fresh log: {e}"))?;

        Ok(())
    }
}

// ── Convenience: global singleton ───────────────────────────────────────────

static GLOBAL_STORE: std::sync::LazyLock<LogStore> =
    std::sync::LazyLock::new(|| LogStore::new(LogStore::default_path()));

/// Append a structured log entry to the global store.
pub fn log_event(level: &str, source: &str, message: &str, session_id: Option<&str>) {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let entry = LogEntry {
        timestamp_ms: now_ms,
        level: level.to_string(),
        source: source.to_string(),
        message: message.to_string(),
        session_id: session_id.map(|s| s.to_string()),
    };

    if GLOBAL_STORE.append(&entry).is_err() {
        eprintln!("Forge structured log entry suppressed: persistence failed");
    }
}

/// Read recent entries from the global store.
pub fn read_recent_logs(limit: usize, level_filter: Option<&str>) -> Result<Vec<LogEntry>, String> {
    GLOBAL_STORE.read_recent(limit, level_filter)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forge-log-{name}-{nanos}.log"))
    }

    fn info_entry(msg: &str) -> LogEntry {
        LogEntry {
            timestamp_ms: 1000,
            level: "INFO".into(),
            source: "test".into(),
            message: msg.into(),
            session_id: None,
        }
    }

    fn cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
        for i in 1..=MAX_ROTATIONS {
            let _ = fs::remove_file(path.with_extension(format!("log.{}.gz", i)));
        }
    }

    // ── Append / read ────────────────────────────────────────────────────

    #[test]
    fn append_redacts_sensitive_entry_before_persistence() {
        let path = temp_path("redacted");
        let redactor = std::sync::Arc::new(crate::redaction::PersistentLogRedactor::new());
        redactor.register_secret("forge-persisted-secret");
        let store = LogStore::new_with_redactor(path.clone(), redactor);

        store
            .append(&info_entry(
                "Authorization: Bearer forge-persisted-secret at https://example.test/run?token=forge-persisted-secret",
            ))
            .expect("append redacted entry");

        let persisted = fs::read_to_string(&path).expect("read persisted log");
        assert!(!persisted.contains("forge-persisted-secret"));
        assert!(!persisted.contains("?token="));
        assert!(persisted.contains("[redacted]"));
        cleanup(&path);
    }

    #[test]
    fn structured_redaction_error_suppresses_persistence() {
        let path = temp_path("redaction-error");
        let redactor = std::sync::Arc::new(crate::redaction::PersistentLogRedactor::new());
        redactor.set_fail_for_test(true);
        let store = LogStore::new_with_redactor(path.clone(), redactor);

        let error = store
            .append(&info_entry("must never reach disk"))
            .expect_err("redaction failure must suppress persistence");

        assert_eq!(error, "redaction failed");
        assert!(!path.exists());
        cleanup(&path);
    }

    #[test]
    fn append_and_read_single_entry() {
        let path = temp_path("single");
        let store = LogStore::new(path.clone());

        store.append(&info_entry("hello")).expect("append");
        let entries = store.read_recent(10, None).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "hello");
        assert_eq!(entries[0].level, "INFO");
        assert_eq!(entries[0].source, "test");

        cleanup(&path);
    }

    #[test]
    fn append_multiple_and_read_tail() {
        let path = temp_path("multi");
        let store = LogStore::new(path.clone());

        for i in 0..10 {
            store
                .append(&info_entry(&format!("msg-{i}")))
                .expect("append");
        }

        let entries = store.read_recent(3, None).expect("read");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "msg-7");
        assert_eq!(entries[2].message, "msg-9");

        cleanup(&path);
    }

    #[test]
    fn read_recent_returns_empty_for_missing_file() {
        let path = temp_path("missing");
        let store = LogStore::new(path.clone());
        let entries = store.read_recent(10, None).expect("read");
        assert!(entries.is_empty());
        cleanup(&path);
    }

    // ── Level filter ─────────────────────────────────────────────────────

    #[test]
    fn filter_by_level() {
        let path = temp_path("filter");
        let store = LogStore::new(path.clone());

        store
            .append(&LogEntry {
                timestamp_ms: 1,
                level: "INFO".into(),
                source: "t".into(),
                message: "info msg".into(),
                session_id: None,
            })
            .expect("append");
        store
            .append(&LogEntry {
                timestamp_ms: 2,
                level: "ERROR".into(),
                source: "t".into(),
                message: "error msg".into(),
                session_id: None,
            })
            .expect("append");

        let errors = store.read_recent(10, Some("ERROR")).expect("read");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "error msg");

        let infos = store.read_recent(10, Some("INFO")).expect("read");
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].message, "info msg");

        cleanup(&path);
    }

    // ── Session id ──────────────────────────────────────────────────────

    #[test]
    fn entry_with_session_id_roundtrips() {
        let path = temp_path("session");
        let store = LogStore::new(path.clone());

        store
            .append(&LogEntry {
                timestamp_ms: 1,
                level: "INFO".into(),
                source: "session".into(),
                message: "created".into(),
                session_id: Some("abc-123".into()),
            })
            .expect("append");

        let entries = store.read_recent(10, None).expect("read");
        assert_eq!(entries[0].session_id.as_deref(), Some("abc-123"));

        cleanup(&path);
    }

    // ── Serialization ────────────────────────────────────────────────────

    #[test]
    fn log_entry_serializes_to_valid_json() {
        let entry = LogEntry {
            timestamp_ms: 1718123456789,
            level: "WARN".into(),
            source: "gateway".into(),
            message: "connection lost".into(),
            session_id: None,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("\"level\":\"WARN\""));
        assert!(json.contains("\"source\":\"gateway\""));
        assert!(json.contains("\"message\":\"connection lost\""));
        // session_id should be omitted when None.
        assert!(!json.contains("session_id"));
    }

    #[test]
    fn log_entry_with_session_serializes_session_id() {
        let entry = LogEntry {
            timestamp_ms: 1,
            level: "INFO".into(),
            source: "s".into(),
            message: "m".into(),
            session_id: Some("sid".into()),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("\"session_id\":\"sid\""));
    }

    // ── Rotation ─────────────────────────────────────────────────────────

    #[test]
    fn rotate_shifts_files() {
        let path = temp_path("rotate");
        let store = LogStore::new(path.clone());

        // Write enough to trigger rotation (the rotation trigger is based on
        // file size >= 5MB, so we can't easily test that in a unit test. But
        // we can test the rotate function directly.)
        store.append(&info_entry("before rotate")).expect("append");

        // Call rotate directly.
        store.rotate().expect("rotate");

        let entries = store.read_recent(10, None).expect("read");
        // After rotation, the log should be empty (fresh file).
        assert!(entries.is_empty());

        // Write after rotation.
        store.append(&info_entry("after rotate")).expect("append");
        let entries = store.read_recent(10, None).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "after rotate");

        cleanup(&path);
    }

    // ── LogEntry JSON roundtrip ──────────────────────────────────────────

    #[test]
    fn log_entry_json_roundtrip() {
        let entry = info_entry("roundtrip test");
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: LogEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.message, "roundtrip test");
        assert_eq!(back.level, "INFO");
        assert_eq!(back.source, "test");
        assert_eq!(back.session_id, None);
    }

    #[test]
    fn read_skips_malformed_lines() {
        let path = temp_path("malformed");
        // Write directly to file — valid JSON line + garbage line + valid JSON line.
        fs::write(
            &path,
            "{\"timestamp_ms\":1,\"level\":\"INFO\",\"source\":\"t\",\"message\":\"ok1\"}\nnot-json\n{\"timestamp_ms\":2,\"level\":\"WARN\",\"source\":\"t\",\"message\":\"ok2\"}\n",
        )
        .expect("write");

        let store = LogStore::new(path.clone());
        let entries = store.read_recent(10, None).expect("read");
        // Should have 2 valid entries, skipping the malformed middle line.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "ok1");
        assert_eq!(entries[1].message, "ok2");

        cleanup(&path);
    }
}
