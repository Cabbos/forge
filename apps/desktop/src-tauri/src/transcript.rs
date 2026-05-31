use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::protocol::events::StreamEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptRecord {
    #[serde(default = "default_transcript_protocol_version")]
    protocol_version: u32,
    recorded_at_ms: u64,
    event: serde_json::Value,
}

const TRANSCRIPT_PROTOCOL_VERSION: u32 = 1;
const TRANSCRIPT_MAX_BYTES: u64 = 5_000_000;
const TRANSCRIPT_RETAIN_EVENTS: usize = 2_000;
const TRANSCRIPT_COMPACT_MARKER: &str = "_forge_transcript_compacted";

static TRANSCRIPT_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

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
    if let Err(error) = app_handle.emit("session-output", event) {
        crate::app_log!(
            "WARN",
            "[event_bus] failed to emit session-output event: {error}"
        );
    }
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

    with_transcript_path_lock(&path, || {
        let record = TranscriptRecord {
            protocol_version: TRANSCRIPT_PROTOCOL_VERSION,
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
    })
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
    load_transcript_events_at_with_limits(
        root,
        session_id,
        TRANSCRIPT_MAX_BYTES,
        TRANSCRIPT_RETAIN_EVENTS,
    )
}

fn load_transcript_events_at_with_limits(
    root: &Path,
    session_id: &str,
    max_bytes: u64,
    retain_events: usize,
) -> Result<Vec<serde_json::Value>, String> {
    let path = transcript_path(root, session_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    with_transcript_path_lock(&path, || {
        let should_throttle = max_bytes > 0
            && retain_events > 0
            && fs::metadata(&path)
                .map(|metadata| metadata.len() > max_bytes)
                .unwrap_or(false);
        let records = load_transcript_records_from_path(&path)?;
        let events =
            renderable_events_for_load(records, session_id, should_throttle, retain_events);
        Ok(close_unfinished_renderable_events(events, session_id))
    })
}

fn renderable_events_for_load(
    records: Vec<TranscriptRecord>,
    session_id: &str,
    should_throttle: bool,
    retain_events: usize,
) -> Vec<serde_json::Value> {
    let total_renderable = records
        .iter()
        .filter(|record| !is_compact_marker(&record.event))
        .count();
    let skip_count = if should_throttle && total_renderable > retain_events {
        total_renderable - retain_events
    } else {
        0
    };

    if skip_count > 0 {
        crate::app_log!(
            "WARN",
            "[transcript] session {} has {} persisted events; retaining latest {} for load",
            session_id,
            total_renderable,
            retain_events
        );
    }

    records
        .into_iter()
        .filter(|record| !is_compact_marker(&record.event))
        .skip(skip_count)
        .map(|record| record.event)
        .collect()
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
    let previous_omitted = records
        .iter()
        .map(|record| compact_marker_omitted_events(&record.event))
        .sum::<usize>();
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
    let omitted = previous_omitted.saturating_add(total_renderable.saturating_sub(retained.len()));
    let mut compacted = vec![TranscriptRecord {
        protocol_version: TRANSCRIPT_PROTOCOL_VERSION,
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

fn compact_marker_omitted_events(event: &serde_json::Value) -> usize {
    if !is_compact_marker(event) {
        return 0;
    }
    event
        .get("omitted_events")
        .and_then(|value| value.as_u64())
        .map(|value| value.min(usize::MAX as u64) as usize)
        .unwrap_or(0)
}

fn default_transcript_protocol_version() -> u32 {
    TRANSCRIPT_PROTOCOL_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingRenderableKind {
    Shell,
    ToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRenderable {
    block_id: String,
    kind: PendingRenderableKind,
    tool_result_seen: bool,
}

fn close_unfinished_renderable_events(
    mut events: Vec<serde_json::Value>,
    session_id: &str,
) -> Vec<serde_json::Value> {
    let mut pending = Vec::<PendingRenderable>::new();

    for event in &events {
        let event_type = event.get("event_type").and_then(|value| value.as_str());
        let block_id = event
            .get("block_id")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let Some(block_id) = block_id else {
            continue;
        };

        match event_type {
            Some("shell_start") => upsert_pending(
                &mut pending,
                PendingRenderable {
                    block_id,
                    kind: PendingRenderableKind::Shell,
                    tool_result_seen: false,
                },
            ),
            Some("shell_end") => {
                remove_pending(&mut pending, &block_id, PendingRenderableKind::Shell)
            }
            Some("tool_call_start") => upsert_pending(
                &mut pending,
                PendingRenderable {
                    block_id,
                    kind: PendingRenderableKind::ToolCall,
                    tool_result_seen: false,
                },
            ),
            Some("tool_call_result") => mark_tool_result_seen(&mut pending, &block_id),
            Some("tool_call_end") => {
                remove_pending(&mut pending, &block_id, PendingRenderableKind::ToolCall)
            }
            _ => {}
        }
    }

    for item in pending {
        match item.kind {
            PendingRenderableKind::Shell => {
                events.push(serde_json::json!({
                    "event_type": "shell_output",
                    "session_id": session_id,
                    "block_id": item.block_id,
                    "content": "\n命令已中断：会话恢复时没有收到结束事件，请根据当前项目状态继续。\n",
                }));
                events.push(serde_json::json!({
                    "event_type": "shell_end",
                    "session_id": session_id,
                    "block_id": item.block_id,
                    "exit_code": -1,
                }));
            }
            PendingRenderableKind::ToolCall => {
                if !item.tool_result_seen {
                    events.push(serde_json::json!({
                        "event_type": "tool_call_result",
                        "session_id": session_id,
                        "block_id": item.block_id,
                        "result": "工具调用已中断：会话恢复时没有收到结果，请重新检查当前项目状态后继续。",
                        "is_error": true,
                        "duration_ms": 0,
                    }));
                }
                events.push(serde_json::json!({
                    "event_type": "tool_call_end",
                    "session_id": session_id,
                    "block_id": item.block_id,
                }));
            }
        }
    }

    events
}

fn upsert_pending(pending: &mut Vec<PendingRenderable>, item: PendingRenderable) {
    remove_pending(pending, &item.block_id, item.kind);
    pending.push(item);
}

fn remove_pending(
    pending: &mut Vec<PendingRenderable>,
    block_id: &str,
    kind: PendingRenderableKind,
) {
    pending.retain(|item| !(item.block_id == block_id && item.kind == kind));
}

fn mark_tool_result_seen(pending: &mut [PendingRenderable], block_id: &str) {
    if let Some(item) = pending
        .iter_mut()
        .find(|item| item.block_id == block_id && item.kind == PendingRenderableKind::ToolCall)
    {
        item.tool_result_seen = true;
    }
}

fn with_transcript_path_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let locks = TRANSCRIPT_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let lock = {
        let mut guard = locks
            .lock()
            .map_err(|_| "Transcript lock registry is poisoned".to_string())?;
        guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock
        .lock()
        .map_err(|_| "Transcript path lock is poisoned".to_string())?;
    operation()
}

fn delete_transcript_at(root: &Path, session_id: &str) -> Result<(), String> {
    let path = transcript_path(root, session_id)?;
    with_transcript_path_lock(&path, || {
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                format!("Failed to delete transcript '{}': {error}", session_id)
            })?;
        }
        Ok(())
    })
}

fn transcript_path(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    let id = safe_session_id(session_id);
    if id.is_empty() || id != session_id {
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
    fn transcript_concurrent_appends_preserve_all_records_without_corrupt_lines() {
        let root = std::sync::Arc::new(test_root("concurrent-append"));
        let session_id = "session-1";
        let writers = 8;
        let events_per_writer = 25;
        let handles = (0..writers)
            .map(|writer| {
                let root = root.clone();
                std::thread::spawn(move || {
                    for index in 0..events_per_writer {
                        append_transcript_event_at(
                            &root,
                            json!({
                                "event_type": "text_chunk",
                                "session_id": session_id,
                                "block_id": format!("writer-{writer}-event-{index}"),
                                "content": format!("writer {writer} event {index}")
                            }),
                        )
                        .expect("append event");
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("writer thread");
        }

        let path = transcript_path(&root, session_id).expect("transcript path");
        let persisted = fs::read_to_string(&path).expect("read transcript");
        let persisted_records = persisted
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid jsonl"))
            .collect::<Vec<_>>();
        let loaded = load_transcript_events_at(&root, session_id).expect("load transcript");
        let block_ids = loaded
            .iter()
            .filter_map(|event| event.get("block_id").and_then(|value| value.as_str()))
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(persisted_records.len(), writers * events_per_writer);
        assert!(persisted_records
            .iter()
            .all(|record| record.get("protocol_version") == Some(&json!(1))));
        assert_eq!(loaded.len(), writers * events_per_writer);
        assert_eq!(block_ids.len(), writers * events_per_writer);

        let _ = fs::remove_dir_all(root.as_ref());
    }

    #[test]
    fn transcript_concurrent_appends_with_compaction_preserve_recent_events_and_marker() {
        let root = std::sync::Arc::new(test_root("concurrent-append-compact"));
        let session_id = "session-1";
        let writers = 6;
        let events_per_writer = 20;
        let retain_events = 10;
        let handles = (0..writers)
            .map(|writer| {
                let root = root.clone();
                std::thread::spawn(move || {
                    for index in 0..events_per_writer {
                        append_transcript_event_at_with_limits(
                            &root,
                            json!({
                                "event_type": "text_chunk",
                                "session_id": session_id,
                                "block_id": format!("writer-{writer}-event-{index}"),
                                "content": format!("writer {writer} event {index}")
                            }),
                            1,
                            retain_events,
                        )
                        .expect("append compacting event");
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("writer thread");
        }

        let path = transcript_path(&root, session_id).expect("transcript path");
        let persisted = fs::read_to_string(&path).expect("read transcript");
        let records = persisted
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid jsonl"))
            .collect::<Vec<_>>();
        let first_event = records
            .first()
            .and_then(|record| record.get("event"))
            .expect("compact marker event");
        let loaded = load_transcript_events_at(&root, session_id).expect("load transcript");
        let loaded_ids = loaded
            .iter()
            .filter_map(|event| event.get("block_id").and_then(|value| value.as_str()))
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(records.len(), retain_events + 1);
        assert!(records
            .iter()
            .all(|record| record.get("protocol_version") == Some(&json!(1))));
        assert_eq!(
            first_event
                .get("event_type")
                .and_then(|value| value.as_str()),
            Some(TRANSCRIPT_COMPACT_MARKER)
        );
        assert_eq!(
            first_event.get("omitted_events"),
            Some(&json!(writers * events_per_writer - retain_events))
        );
        assert_eq!(loaded.len(), retain_events);
        assert_eq!(loaded_ids.len(), retain_events);
        assert!(loaded.iter().all(|event| !is_compact_marker(event)));

        let _ = fs::remove_dir_all(root.as_ref());
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
    fn transcript_path_rejects_session_ids_that_would_be_sanitized() {
        let root = test_root("reject-sanitized-id");

        let error = transcript_path(&root, "../session-1")
            .expect_err("session ids must not be silently rewritten");

        assert!(error.contains("Invalid session id"));
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
    fn transcript_records_include_protocol_version_and_load_legacy_records() {
        let root = test_root("protocol-version");
        let event_one = json!({
            "event_type": "text_chunk",
            "session_id": "session-1",
            "block_id": "text-1",
            "content": "new"
        });
        append_transcript_event_at(&root, event_one.clone()).expect("append versioned event");
        let path = transcript_path(&root, "session-1").expect("transcript path");
        let first_line = fs::read_to_string(&path)
            .expect("read transcript")
            .lines()
            .next()
            .expect("first transcript line")
            .to_string();
        let record = serde_json::from_str::<serde_json::Value>(&first_line)
            .expect("parse transcript record");
        assert_eq!(record.get("protocol_version"), Some(&json!(1)));

        let legacy_event = json!({
            "event_type": "text_chunk",
            "session_id": "session-1",
            "block_id": "text-legacy",
            "content": "legacy"
        });
        let legacy_line = serde_json::to_string(&json!({
            "recorded_at_ms": 1,
            "event": legacy_event
        }))
        .expect("encode legacy record");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open transcript")
            .write_all(format!("{legacy_line}\n").as_bytes())
            .expect("write legacy line");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(
            loaded
                .iter()
                .filter_map(|event| event.get("content").and_then(|value| value.as_str()))
                .collect::<Vec<_>>(),
            vec!["new", "legacy"]
        );
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
    fn transcript_load_throttles_legacy_uncompacted_large_files() {
        let root = test_root("load-throttle");
        for index in 0..8 {
            append_transcript_event_at(
                &root,
                json!({
                    "event_type": "text_chunk",
                    "session_id": "session-1",
                    "block_id": format!("text-{index}"),
                    "content": format!("event-{index}")
                }),
            )
            .expect("append legacy-sized event");
        }

        let path = transcript_path(&root, "session-1").expect("transcript path");
        let persisted = fs::read_to_string(&path).expect("read transcript");
        let persisted_records = persisted
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("record json"))
            .collect::<Vec<_>>();
        assert_eq!(persisted_records.len(), 8);
        assert!(persisted_records.iter().all(|record| {
            record
                .get("event")
                .is_some_and(|event| !is_compact_marker(event))
        }));

        let loaded = load_transcript_events_at_with_limits(&root, "session-1", 1, 3)
            .expect("load transcript");
        let contents = loaded
            .iter()
            .filter_map(|event| event.get("content").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(contents, vec!["event-5", "event-6", "event-7"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_compaction_keeps_internal_marker_off_loaded_events() {
        let root = test_root("compact-marker");
        for index in 0..5 {
            append_transcript_event_at_with_limits(
                &root,
                json!({
                    "event_type": "text_chunk",
                    "session_id": "session-1",
                    "block_id": format!("text-{index}"),
                    "content": format!("event-{index}")
                }),
                1,
                2,
            )
            .expect("append compacting event");
        }

        let path = transcript_path(&root, "session-1").expect("transcript path");
        let persisted = fs::read_to_string(&path).expect("read compacted transcript");
        let records = persisted
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("record json"))
            .collect::<Vec<_>>();
        let first_event = records
            .first()
            .and_then(|record| record.get("event"))
            .expect("compact marker event");

        assert!(records
            .iter()
            .all(|record| record.get("protocol_version") == Some(&json!(1))));
        assert_eq!(
            first_event
                .get("event_type")
                .and_then(|value| value.as_str()),
            Some(TRANSCRIPT_COMPACT_MARKER)
        );
        assert_eq!(first_event.get("omitted_events"), Some(&json!(3)));

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");
        assert!(loaded.iter().all(|event| !is_compact_marker(event)));
        assert_eq!(
            loaded
                .iter()
                .filter_map(|event| event.get("content").and_then(|value| value.as_str()))
                .collect::<Vec<_>>(),
            vec!["event-3", "event-4"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_load_closes_unfinished_shell_blocks_as_cancelled() {
        let root = test_root("unfinished-shell");
        let start = json!({
            "event_type": "shell_start",
            "session_id": "session-1",
            "block_id": "shell-1",
            "command": "npm install"
        });
        append_transcript_event_at(&root, start.clone()).expect("append shell start");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(loaded.first(), Some(&start));
        assert_eq!(
            loaded.last(),
            Some(&json!({
                "event_type": "shell_end",
                "session_id": "session-1",
                "block_id": "shell-1",
                "exit_code": -1
            }))
        );
        assert!(loaded.iter().any(|event| {
            event.get("event_type").and_then(|value| value.as_str()) == Some("shell_output")
                && event
                    .get("content")
                    .and_then(|value| value.as_str())
                    .is_some_and(|content| content.contains("已中断"))
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_load_closes_unfinished_tool_calls_as_cancelled() {
        let root = test_root("unfinished-tool");
        let start = json!({
            "event_type": "tool_call_start",
            "session_id": "session-1",
            "block_id": "tool-1",
            "tool_name": "run_shell",
            "tool_input": { "command": "npm install" }
        });
        append_transcript_event_at(&root, start.clone()).expect("append tool start");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(loaded.first(), Some(&start));
        assert!(loaded.iter().any(|event| {
            event.get("event_type").and_then(|value| value.as_str()) == Some("tool_call_result")
                && event
                    .get("is_error")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
                && event
                    .get("result")
                    .and_then(|value| value.as_str())
                    .is_some_and(|result| result.contains("已中断"))
        }));
        assert_eq!(
            loaded.last(),
            Some(&json!({
                "event_type": "tool_call_end",
                "session_id": "session-1",
                "block_id": "tool-1"
            }))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transcript_load_closes_tool_calls_that_have_result_but_missing_end() {
        let root = test_root("unfinished-tool-end");
        let start = json!({
            "event_type": "tool_call_start",
            "session_id": "session-1",
            "block_id": "tool-1",
            "tool_name": "read_file",
            "tool_input": { "path": "src/App.tsx" }
        });
        let result = json!({
            "event_type": "tool_call_result",
            "session_id": "session-1",
            "block_id": "tool-1",
            "result": "ok",
            "is_error": false,
            "duration_ms": 12
        });
        append_transcript_event_at(&root, start.clone()).expect("append tool start");
        append_transcript_event_at(&root, result.clone()).expect("append tool result");

        let loaded = load_transcript_events_at(&root, "session-1").expect("load transcript");

        assert_eq!(loaded.first(), Some(&start));
        assert_eq!(loaded.get(1), Some(&result));
        assert_eq!(
            loaded.last(),
            Some(&json!({
                "event_type": "tool_call_end",
                "session_id": "session-1",
                "block_id": "tool-1"
            }))
        );
        assert_eq!(
            loaded
                .iter()
                .filter(|event| {
                    event.get("event_type").and_then(|value| value.as_str())
                        == Some("tool_call_result")
                })
                .count(),
            1,
            "existing result should not be duplicated during recovery"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compacted_transcript_load_closes_tool_calls_with_result_but_missing_end() {
        let root = test_root("compact-unfinished-tool-end");
        let session_id = "session-1";
        for index in 0..4 {
            append_transcript_event_at_with_limits(
                &root,
                json!({
                    "event_type": "text_chunk",
                    "session_id": session_id,
                    "block_id": format!("old-{index}"),
                    "content": format!("old event {index}")
                }),
                1,
                2,
            )
            .expect("append compacting old event");
        }
        let start = json!({
            "event_type": "tool_call_start",
            "session_id": session_id,
            "block_id": "tool-1",
            "tool_name": "read_file",
            "tool_input": { "path": "src/App.tsx" }
        });
        let result = json!({
            "event_type": "tool_call_result",
            "session_id": session_id,
            "block_id": "tool-1",
            "result": "ok",
            "is_error": false,
            "duration_ms": 12
        });
        append_transcript_event_at_with_limits(&root, start.clone(), 1, 2)
            .expect("append compacting tool start");
        append_transcript_event_at_with_limits(&root, result.clone(), 1, 2)
            .expect("append compacting tool result");

        let loaded = load_transcript_events_at(&root, session_id).expect("load transcript");

        assert!(loaded.iter().all(|event| !is_compact_marker(event)));
        assert_eq!(loaded.first(), Some(&start));
        assert_eq!(loaded.get(1), Some(&result));
        assert_eq!(
            loaded.last(),
            Some(&json!({
                "event_type": "tool_call_end",
                "session_id": session_id,
                "block_id": "tool-1"
            }))
        );

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
