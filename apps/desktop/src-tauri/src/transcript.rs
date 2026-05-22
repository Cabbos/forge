use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, sync::MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::protocol::events::StreamEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptRecord {
    recorded_at_ms: u64,
    event: serde_json::Value,
}

const TRANSCRIPT_MAX_BYTES: u64 = 5_000_000;
const TRANSCRIPT_RETAIN_EVENTS: usize = 2_000;
const TRANSCRIPT_COMPACT_MARKER: &str = "_forge_transcript_compacted";

static TRANSCRIPT_LOCKS: OnceLock<Mutex<HashMap<PathBuf, &'static Mutex<()>>>> = OnceLock::new();

#[tauri::command]
pub async fn load_session_transcript(session_id: String) -> Result<Vec<serde_json::Value>, String> {
    load_transcript_events(&session_id)
}

pub fn append_transcript_event(event: serde_json::Value) -> Result<(), String> {
    append_transcript_event_at(&app_data_dir(), event)
}

pub fn append_stream_event(event: &StreamEvent) -> Result<(), String> {
    append_stream_event_at(&app_data_dir(), event)
}

pub fn emit_stream_event(app_handle: &tauri::AppHandle, event: StreamEvent) {
    if let Err(error) = append_stream_event(&event) {
        crate::app_log!("WARN", "[transcript] {}", error);
    }
    let _ = app_handle.emit("session-output", event);
}

pub fn load_transcript_events(session_id: &str) -> Result<Vec<serde_json::Value>, String> {
    load_transcript_events_at(&app_data_dir(), session_id)
}

pub fn delete_transcript(session_id: &str) -> Result<(), String> {
    delete_transcript_at(&app_data_dir(), session_id)
}

fn append_transcript_event_at(root: &Path, event: serde_json::Value) -> Result<(), String> {
    append_transcript_event_at_with_limits(
        root,
        event,
        TRANSCRIPT_MAX_BYTES,
        TRANSCRIPT_RETAIN_EVENTS,
    )
}

fn append_transcript_event_at_with_limits(
    root: &Path,
    event: serde_json::Value,
    max_bytes: u64,
    retain_events: usize,
) -> Result<(), String> {
    let session_id = event
        .get("session_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Transcript event is missing session_id".to_string())?;
    let path = transcript_path(root, &session_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create transcript dir: {error}"))?;
    }

    let _guard = lock_transcript_path(&path);
    let record = TranscriptRecord {
        recorded_at_ms: now_ms(),
        event,
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Failed to open transcript: {error}"))?;
    let line = serde_json::to_string(&record)
        .map_err(|error| format!("Failed to serialize transcript event: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("Failed to write transcript: {error}"))?;
    compact_transcript_if_needed(&path, &session_id, max_bytes, retain_events)?;
    Ok(())
}

fn append_stream_event_at(root: &Path, event: &StreamEvent) -> Result<(), String> {
    let value = serde_json::to_value(event)
        .map_err(|error| format!("Failed to serialize stream event: {error}"))?;
    append_transcript_event_at(root, value)
}

fn load_transcript_events_at(
    root: &Path,
    session_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let path = transcript_path(root, session_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let _guard = lock_transcript_path(&path);
    let records = load_transcript_records_from_path(&path)?;
    Ok(records
        .into_iter()
        .filter(|record| !is_compact_marker(&record.event))
        .map(|record| record.event)
        .collect())
}

fn load_transcript_records_from_path(path: &Path) -> Result<Vec<TranscriptRecord>, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("Failed to open transcript: {error}"))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.map_err(|error| format!("Failed to read transcript: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<TranscriptRecord>(&line) {
            Ok(record) => records.push(record),
            Err(error) => {
                crate::app_log!(
                    "WARN",
                    "[transcript] skipped invalid JSONL record {} in {}: {}",
                    index + 1,
                    path.display(),
                    error
                );
            }
        }
    }
    Ok(records)
}

fn compact_transcript_if_needed(
    path: &Path,
    session_id: &str,
    max_bytes: u64,
    retain_events: usize,
) -> Result<(), String> {
    if max_bytes == 0 || retain_events == 0 {
        return Ok(());
    }
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= max_bytes {
        return Ok(());
    }

    let records = load_transcript_records_from_path(path)?;
    let total_renderable = records
        .iter()
        .filter(|record| !is_compact_marker(&record.event))
        .count();
    let mut retained = records
        .into_iter()
        .filter(|record| !is_compact_marker(&record.event))
        .rev()
        .take(retain_events)
        .collect::<Vec<_>>();
    retained.reverse();
    let omitted = total_renderable.saturating_sub(retained.len());
    let mut compacted = vec![TranscriptRecord {
        recorded_at_ms: now_ms(),
        event: serde_json::json!({
            "event_type": TRANSCRIPT_COMPACT_MARKER,
            "session_id": session_id,
            "omitted_events": omitted,
        }),
    }];
    compacted.extend(retained);
    rewrite_transcript_records(path, &compacted)
}

fn rewrite_transcript_records(path: &Path, records: &[TranscriptRecord]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("Failed to compact transcript: {error}"))?;
    for record in records {
        let line = serde_json::to_string(record)
            .map_err(|error| format!("Failed to serialize compacted transcript: {error}"))?;
        writeln!(file, "{line}")
            .map_err(|error| format!("Failed to write compacted transcript: {error}"))?;
    }
    Ok(())
}

fn is_compact_marker(event: &serde_json::Value) -> bool {
    event.get("event_type").and_then(|value| value.as_str()) == Some(TRANSCRIPT_COMPACT_MARKER)
}

fn lock_transcript_path(path: &Path) -> MutexGuard<'static, ()> {
    let locks = TRANSCRIPT_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let lock = {
        let mut guard = locks.lock().unwrap();
        *guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Box::leak(Box::new(Mutex::new(()))))
    };
    lock.lock().unwrap()
}

fn delete_transcript_at(root: &Path, session_id: &str) -> Result<(), String> {
    let path = transcript_path(root, session_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Failed to delete transcript '{}': {error}", session_id))?;
    }
    Ok(())
}

fn transcript_path(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    let id = safe_session_id(session_id);
    if id.is_empty() {
        return Err("Invalid session id".to_string());
    }
    Ok(root.join("session-transcripts").join(format!("{id}.jsonl")))
}

fn safe_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn app_data_dir() -> PathBuf {
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    #[test]
    fn transcript_appends_and_loads_events_in_order() {
        let root = test_root("append-load");
        let event_one = json!({
            "event_type": "user_message",
            "session_id": "session-1",
            "block_id": "user-1",
            "content": "hello"
        });
        let event_two = json!({
            "event_type": "text_chunk",
            "session_id": "session-1",
            "block_id": "text-1",
            "content": "world"
        });

        append_transcript_event_at(&root, event_one.clone()).expect("append first event");
        append_transcript_event_at(&root, event_two.clone()).expect("append second event");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(loaded, vec![event_one, event_two]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_delete_removes_session_events() {
        let root = test_root("delete");
        let event = json!({
            "event_type": "user_message",
            "session_id": "session-1",
            "block_id": "user-1",
            "content": "hello"
        });
        append_transcript_event_at(&root, event).expect("append event");

        delete_transcript_at(&root, "session-1").expect("delete transcript");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");
        assert!(loaded.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_load_skips_invalid_jsonl_records() {
        let root = test_root("skip-invalid");
        let event_one = json!({
            "event_type": "text_chunk",
            "session_id": "session-1",
            "block_id": "text-1",
            "content": "hello"
        });
        let event_two = json!({
            "event_type": "text_chunk",
            "session_id": "session-1",
            "block_id": "text-2",
            "content": "world"
        });
        append_transcript_event_at(&root, event_one.clone()).expect("append first event");
        let path = transcript_path(&root, "session-1").expect("transcript path");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open transcript")
            .write_all(b"{broken json\n")
            .expect("write broken line");
        append_transcript_event_at(&root, event_two.clone()).expect("append second event");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(loaded, vec![event_one, event_two]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_compaction_retains_recent_renderable_events() {
        let root = test_root("compact");
        for index in 0..8 {
            append_transcript_event_at_with_limits(
                &root,
                json!({
                    "event_type": "text_chunk",
                    "session_id": "session-1",
                    "block_id": format!("text-{index}"),
                    "content": format!("event-{index}")
                }),
                1,
                3,
            )
            .expect("append compacting event");
        }

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");
        let contents = loaded
            .iter()
            .filter_map(|event| event.get("content").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(contents, vec!["event-5", "event-6", "event-7"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_stream_event_is_recorded_before_frontend_projection() {
        let root = test_root("typed-event");
        let event = crate::protocol::events::StreamEvent::TextChunk {
            session_id: "session-1".to_string(),
            block_id: "text-1".to_string(),
            content: "hello".to_string(),
        };

        append_stream_event_at(&root, &event).expect("append typed event");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");
        assert_eq!(
            loaded,
            vec![json!({
                "event_type": "text_chunk",
                "session_id": "session-1",
                "block_id": "text-1",
                "content": "hello"
            })]
        );
        let _ = fs::remove_dir_all(root);
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forge-transcript-test-{name}-{}",
            uuid::Uuid::now_v7()
        ))
    }
}
