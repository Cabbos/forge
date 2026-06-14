//! Webhook/trigger endpoint — lightweight TCP listener that accepts JSON-line
//! messages on `127.0.0.1:2021` and stores them as pending triggers.
//!
//! Each incoming message is a JSON object with at least a `message` field.
//! The gateway records the trigger and makes it available to the desktop app
//! via `list_pending_triggers`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// A pending trigger received via the webhook endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTrigger {
    pub id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub received_at_ms: u64,
}

/// Thread-safe store for pending triggers.
#[derive(Debug, Default)]
pub struct TriggerStore {
    triggers: Mutex<Vec<PendingTrigger>>,
    path: Option<PathBuf>,
}

impl TriggerStore {
    pub fn new() -> Self {
        Self {
            triggers: Mutex::new(Vec::new()),
            path: None,
        }
    }

    pub fn persistent_default() -> Self {
        Self::persistent_at(default_trigger_store_path())
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self {
            triggers: Mutex::new(load_triggers(&path)),
            path: Some(path),
        }
    }

    /// Push a new trigger.
    pub fn push(&self, trigger: PendingTrigger) {
        if let Ok(mut list) = self.triggers.lock() {
            list.push(trigger);
            self.save_locked(&list);
        }
    }

    /// Drain all pending triggers (for pickup by desktop app).
    pub fn drain(&self) -> Vec<PendingTrigger> {
        self.triggers
            .lock()
            .map(|mut list| {
                let drained = std::mem::take(&mut *list);
                self.save_locked(&list);
                drained
            })
            .unwrap_or_default()
    }

    /// Peek at pending triggers without removing them.
    pub fn list(&self) -> Vec<PendingTrigger> {
        self.triggers
            .lock()
            .map(|list| list.clone())
            .unwrap_or_default()
    }

    fn save_locked(&self, list: &[PendingTrigger]) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = save_triggers(path, list) {
            log::warn!("failed to persist gateway triggers: {error}");
        }
    }
}

fn default_trigger_store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".forge")
        .join("pending-triggers.json")
}

fn load_triggers(path: &Path) -> Vec<PendingTrigger> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<PendingTrigger>>(&raw) {
        Ok(triggers) => triggers,
        Err(error) => {
            log::warn!("failed to load gateway triggers from disk: {error}");
            Vec::new()
        }
    }
}

fn save_triggers(path: &Path, triggers: &[PendingTrigger]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create trigger dir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(triggers).map_err(|e| format!("serialize triggers: {e}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write trigger tmp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace trigger store: {e}"))?;
    Ok(())
}

/// Default TCP port for the webhook endpoint.
pub const WEBHOOK_PORT: u16 = 2021;

/// Start the webhook TCP listener on `127.0.0.1:{WEBHOOK_PORT}`.
/// Runs forever; call `tokio::spawn` to run in background.
pub async fn serve(trigger_store: std::sync::Arc<TriggerStore>) -> Result<(), String> {
    let addr = format!("127.0.0.1:{WEBHOOK_PORT}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("bind webhook tcp: {e}"))?;

    log::info!("Webhook endpoint listening on tcp://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let store = std::sync::Arc::clone(&trigger_store);
                tokio::spawn(async move {
                    handle_webhook_connection(store, stream).await;
                    log::debug!("webhook connection from {peer} closed");
                });
            }
            Err(e) => {
                log::error!("webhook accept error: {e}");
            }
        }
    }
}

async fn handle_webhook_connection(
    store: std::sync::Arc<TriggerStore>,
    stream: tokio::net::TcpStream,
) {
    let (reader, mut writer) = stream.into_split();
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(json) => {
                let message = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if message.is_empty() {
                    let _ = writer
                        .write_all(b"{\"error\":\"missing 'message' field\"}\n")
                        .await;
                    continue;
                }

                let trigger = PendingTrigger {
                    id: uuid::Uuid::now_v7().simple().to_string(),
                    message,
                    profile_id: json
                        .get("profile_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    provider: json
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    model: json
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    received_at_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                };

                let id = trigger.id.clone();
                store.push(trigger);

                let ack = serde_json::json!({"ok":true,"id":id});
                let _ = writer
                    .write_all(format!("{}\n", serde_json::to_string(&ack).unwrap()).as_bytes())
                    .await;
            }
            Err(e) => {
                let _ = writer
                    .write_all(format!("{{\"error\":\"invalid json: {e}\"}}\n").as_bytes())
                    .await;
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_store_push_and_drain() {
        let store = TriggerStore::new();
        assert!(store.list().is_empty());

        store.push(PendingTrigger {
            id: "1".into(),
            message: "hello".into(),
            profile_id: None,
            provider: None,
            model: None,
            received_at_ms: 1000,
        });

        assert_eq!(store.list().len(), 1);

        let drained = store.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].message, "hello");

        // After drain, store is empty.
        assert!(store.list().is_empty());
    }

    #[test]
    fn trigger_store_multiple_drain() {
        let store = TriggerStore::new();
        store.push(PendingTrigger {
            id: "1".into(),
            message: "a".into(),
            profile_id: None,
            provider: None,
            model: None,
            received_at_ms: 1,
        });
        store.push(PendingTrigger {
            id: "2".into(),
            message: "b".into(),
            profile_id: Some("work".into()),
            provider: None,
            model: None,
            received_at_ms: 2,
        });

        let drained = store.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[1].profile_id.as_deref(), Some("work"));
    }

    #[test]
    fn persistent_trigger_store_reloads_pending_triggers_from_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("pending-triggers.json");
        let store = TriggerStore::persistent_at(path.clone());

        store.push(PendingTrigger {
            id: "persisted-1".into(),
            message: "run the morning check".into(),
            profile_id: Some("ops".into()),
            provider: Some("codex".into()),
            model: Some("gpt-5".into()),
            received_at_ms: 10,
        });

        let restored = TriggerStore::persistent_at(path);
        let triggers = restored.list();

        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].id, "persisted-1");
        assert_eq!(triggers[0].profile_id.as_deref(), Some("ops"));
    }

    #[test]
    fn persistent_trigger_store_drain_clears_disk_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("pending-triggers.json");
        let store = TriggerStore::persistent_at(path.clone());

        store.push(PendingTrigger {
            id: "persisted-2".into(),
            message: "ship digest".into(),
            profile_id: None,
            provider: None,
            model: None,
            received_at_ms: 20,
        });

        let drained = store.drain();
        assert_eq!(drained.len(), 1);

        let restored = TriggerStore::persistent_at(path);
        assert!(restored.list().is_empty());
    }

    #[test]
    fn pending_trigger_serialization() {
        let trigger = PendingTrigger {
            id: "abc".into(),
            message: "ship it".into(),
            profile_id: Some("work".into()),
            provider: Some("deepseek".into()),
            model: Some("deepseek-chat".into()),
            received_at_ms: 1718123456789,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        assert!(json.contains("\"message\":\"ship it\""));
        assert!(json.contains("\"profile_id\":\"work\""));
        assert!(json.contains("\"provider\":\"deepseek\""));
    }
}
