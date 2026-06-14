//! Gateway server — Unix-domain-socket listener, request dispatch, and
//! response serialization.

use crate::gateway::protocol::{
    serialize_reply, GatewayError, GatewayErrorBody, GatewayReply, GatewayRequest, GatewayResponse,
    HealthResult, PingResult, GATEWAY_VERSION,
};
use crate::gateway::runner::{TriggerRunRecord, TriggerRunStore};
use crate::gateway::webhook::TriggerStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

// ── Session info ─────────────────────────────────────────────────────────────

/// Lightweight session record tracked by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySessionInfo {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub workspace_path: String,
    pub created_at_ms: u64,
}

/// Gateway runtime snapshot for diagnostics/status surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRuntimeStatus {
    pub ok: bool,
    pub message: String,
    pub uptime_seconds: u64,
    pub active_sessions: usize,
    pub pending_triggers: usize,
    pub claimed_triggers: usize,
    pub dead_letter_runs: usize,
    pub recent_runs: Vec<TriggerRunRecord>,
}

// ── Server state ────────────────────────────────────────────────────────────

/// Shared state accessible to all request handlers.
#[derive(Debug)]
pub struct GatewayState {
    pub started_at: Instant,
    pub active_sessions: Arc<std::sync::atomic::AtomicUsize>,
    sessions: Mutex<HashMap<String, GatewaySessionInfo>>,
    pub trigger_store: Arc<TriggerStore>,
    pub trigger_run_store: Arc<TriggerRunStore>,
}

impl Default for GatewayState {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            trigger_store: Arc::new(TriggerStore::persistent_default()),
            trigger_run_store: Arc::new(TriggerRunStore::persistent_default()),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn active_sessions(&self) -> usize {
        self.active_sessions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Register a new session (called by desktop app / CLI when a session is created).
    pub fn register_session(&self, info: GatewaySessionInfo) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(info.session_id.clone(), info);
            self.active_sessions
                .store(sessions.len(), std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Unregister a session (called when a session ends).
    pub fn unregister_session(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(session_id);
            self.active_sessions
                .store(sessions.len(), std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// List all registered sessions.
    pub fn list_sessions(&self) -> Vec<GatewaySessionInfo> {
        self.sessions
            .lock()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default()
    }
}

/// Socket path for the gateway.
pub fn default_socket_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".forge").join("gateway.sock")
}

// ── Request dispatch ────────────────────────────────────────────────────────

/// Dispatch a single `GatewayRequest` to the appropriate handler and
/// return a `GatewayReply`.
pub fn dispatch(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    match request.method.as_str() {
        "ping" => handle_ping(request.id),
        "health" => handle_health(state, request.id),
        "list_sessions" => handle_list_sessions(state, request.id),
        "register_session" => handle_register_session(state, request),
        "unregister_session" => handle_unregister_session(state, request),
        "list_pending_triggers" => handle_list_triggers(state, request.id),
        "drain_pending_triggers" => handle_drain_triggers(state, request.id),
        "list_trigger_runs" => handle_list_trigger_runs(state, request.id),
        "runtime_status" => handle_runtime_status(state, request.id),
        _ => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32601,
                message: format!("unknown method: {}", request.method),
            },
        }),
    }
}

fn handle_ping(id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(PingResult {
            ok: true,
            gateway_version: GATEWAY_VERSION.to_string(),
        })
        .unwrap(),
    })
}

fn handle_health(state: &GatewayState, id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(HealthResult {
            ok: true,
            uptime_seconds: state.uptime_seconds(),
            active_sessions: state.active_sessions(),
            gateway_version: GATEWAY_VERSION.to_string(),
        })
        .unwrap(),
    })
}

fn handle_list_sessions(state: &GatewayState, id: String) -> GatewayReply {
    let sessions = state.list_sessions();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(sessions).unwrap(),
    })
}

fn handle_register_session(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    match request.params {
        Some(params) => match serde_json::from_value::<GatewaySessionInfo>(params) {
            Ok(info) => {
                state.register_session(info);
                GatewayReply::Ok(GatewayResponse {
                    id: request.id,
                    result: serde_json::json!({"ok": true}),
                })
            }
            Err(e) => GatewayReply::Err(GatewayError {
                id: request.id,
                error: GatewayErrorBody {
                    code: -32602,
                    message: format!("invalid params: {e}"),
                },
            }),
        },
        None => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32602,
                message: "missing params".to_string(),
            },
        }),
    }
}

fn handle_list_triggers(state: &GatewayState, id: String) -> GatewayReply {
    let triggers = state.trigger_store.list();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(triggers).unwrap(),
    })
}

fn handle_drain_triggers(state: &GatewayState, id: String) -> GatewayReply {
    let triggers = state.trigger_store.drain();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(triggers).unwrap(),
    })
}

fn handle_list_trigger_runs(state: &GatewayState, id: String) -> GatewayReply {
    let runs = state.trigger_run_store.list();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(runs).unwrap(),
    })
}

fn handle_runtime_status(state: &GatewayState, id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(build_runtime_status(state)).unwrap(),
    })
}

fn build_runtime_status(state: &GatewayState) -> GatewayRuntimeStatus {
    let triggers = state.trigger_store.list();
    let runs = state.trigger_run_store.list();
    let pending_triggers = triggers
        .iter()
        .filter(|trigger| trigger.claimed_at_ms.is_none())
        .count();
    let claimed_triggers = triggers.len().saturating_sub(pending_triggers);
    let dead_letter_runs = runs
        .iter()
        .filter(|run| run.status == "dead_letter")
        .count();

    GatewayRuntimeStatus {
        ok: true,
        message: "Gateway runtime is reachable.".to_string(),
        uptime_seconds: state.uptime_seconds(),
        active_sessions: state.active_sessions(),
        pending_triggers,
        claimed_triggers,
        dead_letter_runs,
        recent_runs: runs.into_iter().take(20).collect(),
    }
}

fn handle_unregister_session(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    match request.params {
        Some(params) => {
            if let Some(session_id) = params.get("session_id").and_then(|v| v.as_str()) {
                state.unregister_session(session_id);
                GatewayReply::Ok(GatewayResponse {
                    id: request.id,
                    result: serde_json::json!({"ok": true}),
                })
            } else {
                GatewayReply::Err(GatewayError {
                    id: request.id,
                    error: GatewayErrorBody {
                        code: -32602,
                        message: "missing session_id".to_string(),
                    },
                })
            }
        }
        None => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32602,
                message: "missing params".to_string(),
            },
        }),
    }
}

// ── Connection handling ─────────────────────────────────────────────────────

/// Handle a single client connection: read requests line by line, dispatch,
/// and write replies.
pub async fn handle_connection(state: Arc<GatewayState>, stream: UnixStream) {
    let (reader, mut writer) = stream.into_split();
    let buf_reader = BufReader::new(reader);
    let mut lines = buf_reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<GatewayRequest>(&line) {
            Ok(req) => dispatch(&state, req),
            Err(e) => GatewayReply::Err(GatewayError {
                id: "".to_string(),
                error: GatewayErrorBody {
                    code: -32700,
                    message: format!("parse error: {e}"),
                },
            }),
        };
        if let Ok(json) = serialize_reply(&reply) {
            let _ = writer.write_all(json.as_bytes()).await;
        }
    }
}

/// Start the gateway server on the default socket path.
/// Returns immediately; call `.await` on the returned future to block.
pub async fn serve(state: Arc<GatewayState>, socket_path: PathBuf) -> Result<(), String> {
    // Remove stale socket file if it exists.
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).map_err(|e| format!("remove stale socket: {e}"))?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }

    let listener = UnixListener::bind(&socket_path).map_err(|e| format!("bind socket: {e}"))?;

    log::info!("Gateway listening on {}", socket_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    handle_connection(state, stream).await;
                });
            }
            Err(e) => {
                log::error!("accept error: {e}");
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    // ── dispatch ──────────────────────────────────────────────────────────

    #[test]
    fn dispatch_ping_returns_ok() {
        let state = GatewayState::new();
        let req = GatewayRequest {
            id: "1".into(),
            method: "ping".into(),
            params: None,
        };
        let reply = dispatch(&state, req);
        match reply {
            GatewayReply::Ok(resp) => {
                assert_eq!(resp.id, "1");
                let ping: PingResult =
                    serde_json::from_value(resp.result).expect("parse ping result");
                assert!(ping.ok);
                assert!(!ping.gateway_version.is_empty());
            }
            _ => panic!("expected Ok reply, got Err"),
        }
    }

    #[test]
    fn dispatch_health_returns_state() {
        let state = GatewayState::new();
        std::thread::sleep(Duration::from_millis(10));
        let req = GatewayRequest {
            id: "2".into(),
            method: "health".into(),
            params: None,
        };
        let reply = dispatch(&state, req);
        match reply {
            GatewayReply::Ok(resp) => {
                assert_eq!(resp.id, "2");
                let health: HealthResult =
                    serde_json::from_value(resp.result).expect("parse health result");
                assert!(health.ok);
                // uptime_seconds is u64, so non-negativity is guaranteed
                assert_eq!(health.active_sessions, 0);
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn dispatch_unknown_method_returns_error() {
        let state = GatewayState::new();
        let req = GatewayRequest {
            id: "3".into(),
            method: "nonexistent".into(),
            params: None,
        };
        let reply = dispatch(&state, req);
        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.id, "3");
                assert_eq!(err.error.code, -32601);
                assert!(err.error.message.contains("unknown method"));
            }
            _ => panic!("expected Err reply"),
        }
    }

    #[test]
    fn dispatch_list_trigger_runs_returns_records() {
        let state = GatewayState {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            trigger_store: Arc::new(crate::gateway::webhook::TriggerStore::new()),
            trigger_run_store: Arc::new(crate::gateway::runner::TriggerRunStore::new()),
        };
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-1".into(),
                trigger_id: "trigger-1".into(),
                attempt: 1,
                status: "completed".into(),
                message: "ledger ok".into(),
                started_at_ms: 1,
                ended_at_ms: 2,
            });

        let req = GatewayRequest {
            id: "runs".into(),
            method: "list_trigger_runs".into(),
            params: None,
        };
        let reply = dispatch(&state, req);

        match reply {
            GatewayReply::Ok(resp) => {
                let runs: Vec<crate::gateway::runner::TriggerRunRecord> =
                    serde_json::from_value(resp.result).expect("parse trigger runs");
                assert_eq!(runs.len(), 1);
                assert_eq!(runs[0].trigger_id, "trigger-1");
                assert_eq!(runs[0].status, "completed");
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn dispatch_runtime_status_returns_queue_and_run_summary() {
        let state = GatewayState {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            trigger_store: Arc::new(crate::gateway::webhook::TriggerStore::new()),
            trigger_run_store: Arc::new(crate::gateway::runner::TriggerRunStore::new()),
        };
        state.trigger_store.push(test_trigger("pending-1", None));
        state
            .trigger_store
            .push(test_trigger("claimed-1", Some(1234)));
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-dead".into(),
                trigger_id: "claimed-1".into(),
                attempt: 3,
                status: "dead_letter".into(),
                message: "provider offline".into(),
                started_at_ms: 10,
                ended_at_ms: 11,
            });
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-ok".into(),
                trigger_id: "pending-1".into(),
                attempt: 1,
                status: "completed".into(),
                message: "ok".into(),
                started_at_ms: 20,
                ended_at_ms: 21,
            });

        let req = GatewayRequest {
            id: "runtime".into(),
            method: "runtime_status".into(),
            params: None,
        };
        let reply = dispatch(&state, req);

        match reply {
            GatewayReply::Ok(resp) => {
                let status: GatewayRuntimeStatus =
                    serde_json::from_value(resp.result).expect("parse runtime status");
                assert_eq!(status.pending_triggers, 1);
                assert_eq!(status.claimed_triggers, 1);
                assert_eq!(status.dead_letter_runs, 1);
                assert_eq!(status.recent_runs.len(), 2);
                assert_eq!(status.recent_runs[0].id, "run-ok");
            }
            _ => panic!("expected Ok reply"),
        }
    }

    // ── GatewayState ────────────────────────────────────────────────────

    #[test]
    fn gateway_state_starts_with_zero_sessions() {
        let state = GatewayState::new();
        assert_eq!(state.active_sessions(), 0);
    }

    #[test]
    fn gateway_state_registers_and_unregisters_session_count() {
        let state = GatewayState::new();
        state.register_session(test_session("session-1", "claude"));
        assert_eq!(state.active_sessions(), 1);
        assert_eq!(state.list_sessions().len(), 1);

        state.unregister_session("session-1");
        assert_eq!(state.active_sessions(), 0);
        assert!(state.list_sessions().is_empty());
    }

    #[test]
    fn gateway_state_replacing_session_does_not_double_count() {
        let state = GatewayState::new();
        state.register_session(test_session("session-1", "claude"));
        state.register_session(test_session("session-1", "codex"));

        let sessions = state.list_sessions();
        assert_eq!(state.active_sessions(), 1);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider, "codex");
    }

    #[test]
    fn gateway_state_unregistering_missing_session_keeps_count_at_zero() {
        let state = GatewayState::new();
        state.unregister_session("missing-session");

        assert_eq!(state.active_sessions(), 0);
    }

    // ── default_socket_path ─────────────────────────────────────────────

    #[test]
    fn default_socket_path_ends_with_gateway_sock() {
        let path = default_socket_path();
        assert!(path.ends_with("gateway.sock"));
        assert!(path.to_string_lossy().contains(".forge"));
    }

    fn test_session(session_id: &str, provider: &str) -> GatewaySessionInfo {
        GatewaySessionInfo {
            session_id: session_id.to_string(),
            provider: provider.to_string(),
            model: "test-model".to_string(),
            workspace_path: "/tmp/forge-workspace".to_string(),
            created_at_ms: 1,
        }
    }

    fn test_trigger(
        id: &str,
        claimed_at_ms: Option<u64>,
    ) -> crate::gateway::webhook::PendingTrigger {
        crate::gateway::webhook::PendingTrigger {
            id: id.to_string(),
            message: "work".to_string(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
            attempt_count: 0,
            claimed_at_ms,
            received_at_ms: 1,
        }
    }
}
