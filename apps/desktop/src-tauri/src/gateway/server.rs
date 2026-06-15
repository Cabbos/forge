//! Gateway server — Unix-domain-socket listener, request dispatch, and
//! response serialization.

use crate::gateway::protocol::{
    serialize_reply, CancelTriggerParams, CancelTriggerResult, EnqueueTriggerParams,
    EnqueueTriggerResult, GatewayError, GatewayErrorBody, GatewayReply, GatewayRequest,
    GatewayResponse, GetTriggerRunParams, GetTriggerRunResult, HealthResult, PingResult,
    ReplayTriggerRunParams, ReplayTriggerRunResult, GATEWAY_VERSION,
};
use crate::gateway::runner::{TriggerRunRecord, TriggerRunStore};
use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
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
    #[serde(default)]
    pub runtime_tasks: Vec<GatewayRuntimeTaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRuntimeTaskStatus {
    pub name: String,
    pub running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
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
    runtime_tasks: Mutex<HashMap<String, GatewayRuntimeTaskStatus>>,
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
            runtime_tasks: Mutex::new(default_runtime_task_map()),
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

    pub fn mark_runtime_task_started(&self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Ok(mut tasks) = self.runtime_tasks.lock() {
            tasks.insert(
                name.to_string(),
                GatewayRuntimeTaskStatus {
                    name: name.to_string(),
                    running: true,
                    last_started_at_ms: Some(now_millis()),
                    last_error: None,
                },
            );
        }
    }

    pub fn mark_runtime_task_failed(&self, name: &str, error: impl Into<String>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Ok(mut tasks) = self.runtime_tasks.lock() {
            let status =
                tasks
                    .entry(name.to_string())
                    .or_insert_with(|| GatewayRuntimeTaskStatus {
                        name: name.to_string(),
                        running: false,
                        last_started_at_ms: None,
                        last_error: None,
                    });
            status.running = false;
            status.last_error = Some(error.into());
        }
    }

    pub fn runtime_tasks(&self) -> Vec<GatewayRuntimeTaskStatus> {
        let tasks = self
            .runtime_tasks
            .lock()
            .map(|tasks| tasks.clone())
            .unwrap_or_else(|_| default_runtime_task_map());
        ordered_runtime_tasks(tasks)
    }
}

pub const WEBHOOK_LISTENER_TASK: &str = "webhook_listener";
pub const TRIGGER_RUNNER_TASK: &str = "trigger_runner";
pub const SCHEDULER_TICK_TASK: &str = "scheduler_tick";

pub fn default_runtime_task_statuses() -> Vec<GatewayRuntimeTaskStatus> {
    ordered_runtime_tasks(default_runtime_task_map())
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
        "enqueue_trigger" => handle_enqueue_trigger(state, request),
        "cancel_trigger" => handle_cancel_trigger(state, request),
        "replay_trigger_run" => handle_replay_trigger_run(state, request),
        "get_trigger_run" => handle_get_trigger_run(state, request),
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

fn handle_enqueue_trigger(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<EnqueueTriggerParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let message = params.message.trim().to_string();
    if message.is_empty() {
        return invalid_params(request.id, "message must not be empty");
    }

    let trigger_id = params
        .trigger_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(new_trigger_id);

    state.trigger_store.push(PendingTrigger {
        id: trigger_id.clone(),
        message,
        profile_id: clean_optional_string(params.profile_id),
        provider: clean_optional_string(params.provider),
        model: clean_optional_string(params.model),
        workspace_path: clean_optional_string(params.workspace_path),
        attempt_count: 0,
        claimed_at_ms: None,
        received_at_ms: now_millis(),
    });

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(EnqueueTriggerResult {
            ok: true,
            trigger_id,
            pending_triggers: state
                .trigger_store
                .list()
                .iter()
                .filter(|trigger| trigger.claimed_at_ms.is_none())
                .count(),
        })
        .unwrap(),
    })
}

fn handle_cancel_trigger(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CancelTriggerParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let trigger_id = params.trigger_id.trim().to_string();
    if trigger_id.is_empty() {
        return invalid_params(request.id, "trigger_id must not be empty");
    }

    let removed = state.trigger_store.complete(&trigger_id);
    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(CancelTriggerResult {
            ok: true,
            trigger_id,
            removed,
            pending_triggers: count_available_triggers(state),
        })
        .unwrap(),
    })
}

fn handle_replay_trigger_run(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<ReplayTriggerRunParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let run_id = params.run_id.trim().to_string();
    if run_id.is_empty() {
        return invalid_params(request.id, "run_id must not be empty");
    }

    let Some(run) = state.trigger_run_store.find(&run_id) else {
        return invalid_params(request.id, format!("run_id not found: {run_id}"));
    };
    let Some(message) = run
        .trigger_message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string)
    else {
        return invalid_params(
            request.id,
            format!("run {run_id} cannot be replayed because trigger metadata is missing"),
        );
    };

    let trigger_id = new_trigger_id();
    state.trigger_store.push(PendingTrigger {
        id: trigger_id.clone(),
        message,
        profile_id: clean_optional_string(run.profile_id),
        provider: clean_optional_string(run.provider),
        model: clean_optional_string(run.model),
        workspace_path: clean_optional_string(run.workspace_path),
        attempt_count: 0,
        claimed_at_ms: None,
        received_at_ms: now_millis(),
    });

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ReplayTriggerRunResult {
            ok: true,
            run_id,
            trigger_id,
            pending_triggers: count_available_triggers(state),
        })
        .unwrap(),
    })
}

fn handle_get_trigger_run(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GetTriggerRunParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let run_id = params.run_id.trim().to_string();
    if run_id.is_empty() {
        return invalid_params(request.id, "run_id must not be empty");
    }

    let Some(run) = state.trigger_run_store.find(&run_id) else {
        return invalid_params(request.id, format!("run_id not found: {run_id}"));
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(GetTriggerRunResult { ok: true, run }).unwrap(),
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
    let pending_triggers = count_pending_triggers(&triggers);
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
        runtime_tasks: state.runtime_tasks(),
    }
}

fn count_available_triggers(state: &GatewayState) -> usize {
    count_pending_triggers(&state.trigger_store.list())
}

fn count_pending_triggers(triggers: &[PendingTrigger]) -> usize {
    triggers
        .iter()
        .filter(|trigger| trigger.claimed_at_ms.is_none())
        .count()
}

fn default_runtime_task_map() -> HashMap<String, GatewayRuntimeTaskStatus> {
    [
        WEBHOOK_LISTENER_TASK,
        TRIGGER_RUNNER_TASK,
        SCHEDULER_TICK_TASK,
    ]
    .into_iter()
    .map(|name| {
        (
            name.to_string(),
            GatewayRuntimeTaskStatus {
                name: name.to_string(),
                running: false,
                last_started_at_ms: None,
                last_error: None,
            },
        )
    })
    .collect()
}

fn ordered_runtime_tasks(
    tasks: HashMap<String, GatewayRuntimeTaskStatus>,
) -> Vec<GatewayRuntimeTaskStatus> {
    let mut ordered = Vec::with_capacity(tasks.len());
    for name in [
        WEBHOOK_LISTENER_TASK,
        TRIGGER_RUNNER_TASK,
        SCHEDULER_TICK_TASK,
    ] {
        if let Some(status) = tasks.get(name) {
            ordered.push(status.clone());
        }
    }

    let mut extras = tasks
        .into_iter()
        .filter(|(name, _)| {
            ![
                WEBHOOK_LISTENER_TASK,
                TRIGGER_RUNNER_TASK,
                SCHEDULER_TICK_TASK,
            ]
            .contains(&name.as_str())
        })
        .map(|(_, status)| status)
        .collect::<Vec<_>>();
    extras.sort_by(|a, b| a.name.cmp(&b.name));
    ordered.extend(extras);
    ordered
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

fn invalid_params(id: String, message: impl Into<String>) -> GatewayReply {
    GatewayReply::Err(GatewayError {
        id,
        error: GatewayErrorBody {
            code: -32602,
            message: message.into(),
        },
    })
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn new_trigger_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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

    fn test_gateway_state() -> GatewayState {
        GatewayState {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            trigger_store: Arc::new(crate::gateway::webhook::TriggerStore::new()),
            trigger_run_store: Arc::new(crate::gateway::runner::TriggerRunStore::new()),
            runtime_tasks: Mutex::new(default_runtime_task_map()),
        }
    }

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
        let state = test_gateway_state();
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
                trigger_message: None,
                profile_id: None,
                provider: None,
                model: None,
                workspace_path: None,
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
        let state = test_gateway_state();
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
                trigger_message: None,
                profile_id: None,
                provider: None,
                model: None,
                workspace_path: None,
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
                trigger_message: None,
                profile_id: None,
                provider: None,
                model: None,
                workspace_path: None,
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
                assert_eq!(
                    status
                        .runtime_tasks
                        .iter()
                        .map(|task| (task.name.as_str(), task.running))
                        .collect::<Vec<_>>(),
                    [
                        (WEBHOOK_LISTENER_TASK, false),
                        (TRIGGER_RUNNER_TASK, false),
                        (SCHEDULER_TICK_TASK, false),
                    ]
                );
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn runtime_status_reflects_background_task_state() {
        let state = test_gateway_state();
        state.mark_runtime_task_started(TRIGGER_RUNNER_TASK);
        state.mark_runtime_task_failed(WEBHOOK_LISTENER_TASK, "address already in use");

        let status = build_runtime_status(&state);
        let webhook = status
            .runtime_tasks
            .iter()
            .find(|task| task.name == WEBHOOK_LISTENER_TASK)
            .expect("webhook status");
        let trigger = status
            .runtime_tasks
            .iter()
            .find(|task| task.name == TRIGGER_RUNNER_TASK)
            .expect("trigger status");

        assert!(!webhook.running);
        assert_eq!(
            webhook.last_error.as_deref(),
            Some("address already in use")
        );
        assert!(trigger.running);
        assert!(trigger.last_started_at_ms.is_some());
        assert!(trigger.last_error.is_none());
    }

    #[test]
    fn dispatch_enqueue_trigger_pushes_to_store_and_updates_runtime_status() {
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "enqueue".into(),
                method: "enqueue_trigger".into(),
                params: Some(serde_json::json!({
                    "trigger_id": "trigger-ipc-1",
                    "message": "  run digest  ",
                    "profile_id": "ops",
                    "provider": "openai",
                    "model": "gpt-5",
                    "workspace_path": "/tmp/forge-workspace"
                })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::EnqueueTriggerResult =
                    serde_json::from_value(resp.result).expect("parse enqueue result");
                assert!(result.ok);
                assert_eq!(result.trigger_id, "trigger-ipc-1");
                assert_eq!(result.pending_triggers, 1);
            }
            _ => panic!("expected Ok reply"),
        }

        let queued = state.trigger_store.list();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "trigger-ipc-1");
        assert_eq!(queued[0].message, "run digest");
        assert_eq!(queued[0].profile_id.as_deref(), Some("ops"));
        assert_eq!(queued[0].provider.as_deref(), Some("openai"));
        assert_eq!(queued[0].model.as_deref(), Some("gpt-5"));
        assert_eq!(
            queued[0].workspace_path.as_deref(),
            Some("/tmp/forge-workspace")
        );

        let status = build_runtime_status(&state);
        assert_eq!(status.pending_triggers, 1);
        assert_eq!(status.claimed_triggers, 0);
    }

    #[test]
    fn dispatch_enqueue_trigger_rejects_blank_message() {
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "enqueue".into(),
                method: "enqueue_trigger".into(),
                params: Some(serde_json::json!({"message": "   "})),
            },
        );

        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.error.code, -32602);
                assert!(err.error.message.contains("message"));
            }
            _ => panic!("expected Err reply"),
        }
        assert!(state.trigger_store.list().is_empty());
    }

    #[test]
    fn dispatch_cancel_trigger_removes_pending_trigger() {
        let state = test_gateway_state();
        state
            .trigger_store
            .push(test_trigger("trigger-cancel", None));

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "cancel".into(),
                method: "cancel_trigger".into(),
                params: Some(serde_json::json!({"trigger_id": " trigger-cancel "})),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::CancelTriggerResult =
                    serde_json::from_value(resp.result).expect("parse cancel result");
                assert!(result.ok);
                assert!(result.removed);
                assert_eq!(result.trigger_id, "trigger-cancel");
                assert_eq!(result.pending_triggers, 0);
            }
            _ => panic!("expected Ok reply"),
        }
        assert!(state.trigger_store.list().is_empty());
    }

    #[test]
    fn dispatch_cancel_trigger_reports_missing_trigger_without_mutating_queue() {
        let state = test_gateway_state();
        state.trigger_store.push(test_trigger("trigger-keep", None));

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "cancel-missing".into(),
                method: "cancel_trigger".into(),
                params: Some(serde_json::json!({"trigger_id": "missing-trigger"})),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::CancelTriggerResult =
                    serde_json::from_value(resp.result).expect("parse cancel result");
                assert!(result.ok);
                assert!(!result.removed);
                assert_eq!(result.trigger_id, "missing-trigger");
                assert_eq!(result.pending_triggers, 1);
            }
            _ => panic!("expected Ok reply"),
        }
        assert_eq!(state.trigger_store.list().len(), 1);
    }

    #[test]
    fn dispatch_cancel_trigger_rejects_blank_id() {
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "cancel-blank".into(),
                method: "cancel_trigger".into(),
                params: Some(serde_json::json!({"trigger_id": "  "})),
            },
        );

        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.error.code, -32602);
                assert!(err.error.message.contains("trigger_id"));
            }
            _ => panic!("expected Err reply"),
        }
    }

    #[test]
    fn dispatch_replay_trigger_run_queues_new_trigger_from_run_metadata() {
        let state = test_gateway_state();
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-replayable".into(),
                trigger_id: "trigger-original".into(),
                attempt: 2,
                status: "dead_letter".into(),
                message: "provider offline".into(),
                started_at_ms: 10,
                ended_at_ms: 11,
                trigger_message: Some("run the digest again".into()),
                profile_id: Some("ops".into()),
                provider: Some("openai".into()),
                model: Some("gpt-5".into()),
                workspace_path: Some("/repo/workspace".into()),
            });

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "replay".into(),
                method: "replay_trigger_run".into(),
                params: Some(serde_json::json!({"run_id": " run-replayable "})),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::ReplayTriggerRunResult =
                    serde_json::from_value(resp.result).expect("parse replay result");
                assert!(result.ok);
                assert_eq!(result.run_id, "run-replayable");
                assert_ne!(result.trigger_id, "trigger-original");
                assert_eq!(result.pending_triggers, 1);
            }
            _ => panic!("expected Ok reply"),
        }

        let queued = state.trigger_store.list();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].message, "run the digest again");
        assert_eq!(queued[0].profile_id.as_deref(), Some("ops"));
        assert_eq!(queued[0].provider.as_deref(), Some("openai"));
        assert_eq!(queued[0].model.as_deref(), Some("gpt-5"));
        assert_eq!(queued[0].workspace_path.as_deref(), Some("/repo/workspace"));
        assert_eq!(queued[0].attempt_count, 0);
        assert!(queued[0].claimed_at_ms.is_none());
    }

    #[test]
    fn dispatch_replay_trigger_run_rejects_legacy_run_without_metadata() {
        let state = test_gateway_state();
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-legacy".into(),
                trigger_id: "trigger-legacy".into(),
                attempt: 1,
                status: "completed".into(),
                message: "old record".into(),
                started_at_ms: 10,
                ended_at_ms: 11,
                trigger_message: None,
                profile_id: None,
                provider: None,
                model: None,
                workspace_path: None,
            });

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "replay".into(),
                method: "replay_trigger_run".into(),
                params: Some(serde_json::json!({"run_id": "run-legacy"})),
            },
        );

        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.error.code, -32602);
                assert!(err.error.message.contains("metadata"));
            }
            _ => panic!("expected Err reply"),
        }
        assert!(state.trigger_store.list().is_empty());
    }

    #[test]
    fn dispatch_get_trigger_run_returns_requested_run_detail() {
        let state = test_gateway_state();
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-detail".into(),
                trigger_id: "trigger-detail".into(),
                attempt: 3,
                status: "dead_letter".into(),
                message: "provider offline".into(),
                started_at_ms: 10,
                ended_at_ms: 22,
                trigger_message: Some("run digest".into()),
                profile_id: Some("ops".into()),
                provider: Some("openai".into()),
                model: Some("gpt-5".into()),
                workspace_path: Some("/repo".into()),
            });

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "detail".into(),
                method: "get_trigger_run".into(),
                params: Some(serde_json::json!({"run_id": " run-detail "})),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::GetTriggerRunResult =
                    serde_json::from_value(resp.result).expect("parse detail result");
                assert!(result.ok);
                assert_eq!(result.run.id, "run-detail");
                assert_eq!(result.run.trigger_id, "trigger-detail");
                assert_eq!(result.run.trigger_message.as_deref(), Some("run digest"));
                assert_eq!(result.run.workspace_path.as_deref(), Some("/repo"));
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
