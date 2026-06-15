//! Gateway server — Unix-domain-socket listener, request dispatch, and
//! response serialization.

use crate::gateway::protocol::{
    serialize_reply, AttachSessionParams, AttachSessionResult, CancelTriggerParams,
    CancelTriggerResult, CompleteSessionInputParams, CompleteSessionInputResult,
    EnqueueSessionInputParams, EnqueueSessionInputResult, EnqueueTriggerParams,
    EnqueueTriggerResult, GatewayError, GatewayErrorBody, GatewayReply, GatewayRequest,
    GatewayResponse, GatewaySessionAttachStatus, GatewaySessionControl, GatewaySessionControlPlane,
    GatewaySessionInfo, GatewaySessionSnapshotSummary, GetSessionSnapshotParams,
    GetSessionSnapshotResult, GetTriggerRunParams, GetTriggerRunResult, HealthResult,
    ListSessionInputsParams, ListSessionInputsResult, PingResult, ReplayTriggerRunParams,
    ReplayTriggerRunResult, TailSessionEventsParams, TailSessionEventsResult, GATEWAY_VERSION,
};
use crate::gateway::runner::{TriggerRunRecord, TriggerRunStore};
use crate::gateway::session_input::{
    new_session_input_record, SessionInputCompletionRecord, SessionInputStore,
};
use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// Gateway runtime snapshot for diagnostics/status surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRuntimeStatus {
    pub ok: bool,
    pub message: String,
    pub uptime_seconds: u64,
    pub active_sessions: usize,
    pub pending_triggers: usize,
    #[serde(default)]
    pub pending_session_inputs: usize,
    pub claimed_triggers: usize,
    pub dead_letter_runs: usize,
    pub recent_runs: Vec<TriggerRunRecord>,
    #[serde(default)]
    pub recent_session_inputs: Vec<SessionInputCompletionRecord>,
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

/// Aggregated read-only snapshot for gateway dashboard clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayDashboardSnapshot {
    pub ok: bool,
    pub generated_at_ms: u64,
    pub status: GatewayRuntimeStatus,
    pub sessions: Vec<GatewaySessionInfo>,
    pub queued_triggers: Vec<PendingTrigger>,
    pub recent_runs: Vec<TriggerRunRecord>,
    pub recent_session_inputs: Vec<SessionInputCompletionRecord>,
    pub event_log: Vec<GatewayDashboardEventLogEntry>,
}

/// Compact event line derived from gateway run/input history for dashboards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayDashboardEventLogEntry {
    pub kind: String,
    pub id: String,
    pub message: String,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

// ── Server state ────────────────────────────────────────────────────────────

/// Shared state accessible to all request handlers.
#[derive(Debug)]
pub struct GatewayState {
    pub started_at: Instant,
    pub active_sessions: Arc<std::sync::atomic::AtomicUsize>,
    sessions: Mutex<HashMap<String, GatewaySessionInfo>>,
    session_registry_path: Option<PathBuf>,
    pub trigger_store: Arc<TriggerStore>,
    pub trigger_run_store: Arc<TriggerRunStore>,
    pub session_input_store: Arc<SessionInputStore>,
    runtime_tasks: Mutex<HashMap<String, GatewayRuntimeTaskStatus>>,
    include_snapshot_sessions: bool,
}

impl Default for GatewayState {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayState {
    pub fn new() -> Self {
        Self::new_with_session_registry_path_and_snapshot_listing(
            default_session_registry_path(),
            true,
        )
    }

    pub fn new_with_session_registry_path(session_registry_path: PathBuf) -> Self {
        Self::new_with_session_registry_path_and_snapshot_listing(session_registry_path, false)
    }

    fn new_with_session_registry_path_and_snapshot_listing(
        session_registry_path: PathBuf,
        include_snapshot_sessions: bool,
    ) -> Self {
        let sessions = load_session_registry(&session_registry_path);
        let active_sessions = Arc::new(std::sync::atomic::AtomicUsize::new(active_session_count(
            &sessions,
        )));
        Self {
            started_at: Instant::now(),
            active_sessions,
            sessions: Mutex::new(sessions),
            session_registry_path: Some(session_registry_path),
            trigger_store: Arc::new(TriggerStore::persistent_default()),
            trigger_run_store: Arc::new(TriggerRunStore::persistent_default()),
            session_input_store: Arc::new(SessionInputStore::persistent_default()),
            runtime_tasks: Mutex::new(default_runtime_task_map()),
            include_snapshot_sessions,
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn active_sessions(&self) -> usize {
        match self.sessions.lock() {
            Ok(sessions) => {
                let count = active_session_count(&sessions);
                self.active_sessions
                    .store(count, std::sync::atomic::Ordering::Relaxed);
                count
            }
            Err(_) => self
                .active_sessions
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Register a new session (called by desktop app / CLI when a session is created).
    pub fn register_session(&self, info: GatewaySessionInfo) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(info.session_id.clone(), info);
            self.active_sessions.store(
                active_session_count(&sessions),
                std::sync::atomic::Ordering::Relaxed,
            );
            self.save_session_registry_locked(&sessions);
        }
    }

    /// Unregister a session (called when a session ends).
    pub fn unregister_session(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(session_id);
            self.active_sessions.store(
                active_session_count(&sessions),
                std::sync::atomic::Ordering::Relaxed,
            );
            self.save_session_registry_locked(&sessions);
        }
    }

    /// List all registered sessions.
    pub fn list_sessions(&self) -> Vec<GatewaySessionInfo> {
        let mut sessions = self
            .sessions
            .lock()
            .map(|sessions| sessions.clone())
            .unwrap_or_default();
        if self.include_snapshot_sessions {
            for snapshot_session in snapshot_backed_gateway_sessions() {
                sessions
                    .entry(snapshot_session.session_id.clone())
                    .or_insert(snapshot_session);
            }
        }
        sorted_sessions(sessions.into_values())
    }

    /// Return the gateway's current attachment view for a single session id.
    pub fn attach_session(&self, session_id: &str) -> AttachSessionResult {
        let session_id = session_id.trim().to_string();
        let snapshot = gateway_snapshot_summary_for_session(&session_id);
        let session = self
            .sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(&session_id).cloned());
        let Some(session) = session else {
            return AttachSessionResult {
                ok: false,
                session_id,
                status: GatewaySessionAttachStatus::Missing,
                message: "Session is not registered with the gateway.".to_string(),
                control: session_attach_control(
                    GatewaySessionAttachStatus::Missing,
                    snapshot.is_some(),
                ),
                snapshot,
                session: None,
            };
        };

        let status = session_attach_status_at(&session, now_millis());
        let ok = matches!(status, GatewaySessionAttachStatus::Live);
        AttachSessionResult {
            ok,
            session_id,
            status,
            message: session_attach_message(status).to_string(),
            control: session_attach_control(status, snapshot.is_some()),
            snapshot,
            session: Some(session),
        }
    }

    fn save_session_registry_locked(&self, sessions: &HashMap<String, GatewaySessionInfo>) {
        let Some(path) = &self.session_registry_path else {
            return;
        };
        if let Err(error) = save_session_registry(path, sessions) {
            log::warn!("failed to persist gateway session registry: {error}");
        }
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
pub const DASHBOARD_HTTP_TASK: &str = "dashboard_http";
pub const SESSION_STALE_AFTER_MS: u64 = 5 * 60 * 1000;

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
        "attach_session" => handle_attach_session(state, request),
        "register_session" => handle_register_session(state, request),
        "unregister_session" => handle_unregister_session(state, request),
        "list_pending_triggers" => handle_list_triggers(state, request.id),
        "drain_pending_triggers" => handle_drain_triggers(state, request.id),
        "enqueue_trigger" => handle_enqueue_trigger(state, request),
        "enqueue_session_input" => handle_enqueue_session_input(state, request),
        "list_session_inputs" => handle_list_session_inputs(state, request),
        "complete_session_input" => handle_complete_session_input(state, request),
        "cancel_trigger" => handle_cancel_trigger(state, request),
        "replay_trigger_run" => handle_replay_trigger_run(state, request),
        "get_trigger_run" => handle_get_trigger_run(state, request),
        "get_session_snapshot" => handle_get_session_snapshot(request),
        "tail_session_events" => handle_tail_session_events(request),
        "list_trigger_runs" => handle_list_trigger_runs(state, request.id),
        "runtime_status" => handle_runtime_status(state, request.id),
        "dashboard_snapshot" => handle_dashboard_snapshot(state, request.id),
        _ => GatewayReply::Err(GatewayError {
            id: request.id,
            error: GatewayErrorBody {
                code: -32601,
                message: format!("unknown method: {}", request.method),
            },
        }),
    }
}

fn handle_attach_session(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<AttachSessionParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(state.attach_session(&session_id)).unwrap(),
    })
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

fn handle_enqueue_session_input(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<EnqueueSessionInputParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }
    let message = params.message.trim().to_string();
    if message.is_empty() {
        return invalid_params(request.id, "message must not be empty");
    }

    let input_id = params
        .input_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(new_trigger_id);

    state.session_input_store.push(new_session_input_record(
        input_id.clone(),
        session_id.clone(),
        message,
    ));

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(EnqueueSessionInputResult {
            ok: true,
            input_id,
            session_id,
            pending_inputs: state.session_input_store.list().len(),
        })
        .unwrap(),
    })
}

fn handle_list_session_inputs(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<ListSessionInputsParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_ids = clean_session_ids(params.session_ids);
    if session_ids.is_empty() {
        return invalid_params(request.id, "session_ids must not be empty");
    }
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let inputs = state
        .session_input_store
        .list_for_sessions(&session_ids, limit);

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ListSessionInputsResult {
            ok: true,
            inputs,
            pending_inputs: state.session_input_store.list().len(),
        })
        .unwrap(),
    })
}

fn handle_complete_session_input(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CompleteSessionInputParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let input_id = params.input_id.trim().to_string();
    if input_id.is_empty() {
        return invalid_params(request.id, "input_id must not be empty");
    }
    let removed = state.session_input_store.complete(&input_id);

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(CompleteSessionInputResult {
            ok: true,
            input_id,
            removed,
            pending_inputs: state.session_input_store.list().len(),
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

fn handle_get_session_snapshot(request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GetSessionSnapshotParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    let snapshot = match crate::agent::snapshot::load_session_snapshot(&session_id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return invalid_params(
                request.id,
                format!("session snapshot not available: {error}"),
            );
        }
    };
    let snapshot = match serde_json::to_value(snapshot) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return invalid_params(request.id, format!("serialize snapshot: {error}"));
        }
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(GetSessionSnapshotResult {
            ok: true,
            session_id,
            snapshot,
        })
        .unwrap(),
    })
}

fn handle_tail_session_events(request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<TailSessionEventsParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }

    let tail = match crate::transcript::tail_transcript_events(
        &session_id,
        params.after_cursor,
        params.limit.unwrap_or(100),
    ) {
        Ok(tail) => tail,
        Err(error) => {
            return invalid_params(request.id, format!("session events unavailable: {error}"));
        }
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(TailSessionEventsResult {
            ok: true,
            session_id: tail.session_id,
            events: tail.events,
            next_cursor: tail.next_cursor,
            total_events: tail.total_events,
            cursor_reset: tail.cursor_reset,
        })
        .unwrap(),
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

fn handle_dashboard_snapshot(state: &GatewayState, id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(build_dashboard_snapshot(state)).unwrap(),
    })
}

fn build_dashboard_snapshot(state: &GatewayState) -> GatewayDashboardSnapshot {
    let status = build_runtime_status(state);
    let sessions = state.list_sessions();
    let queued_triggers = state.trigger_store.list();
    let recent_runs = status.recent_runs.clone();
    let recent_session_inputs = status.recent_session_inputs.clone();
    let event_log =
        build_dashboard_event_log(&recent_runs, &recent_session_inputs, &status.runtime_tasks);

    GatewayDashboardSnapshot {
        ok: status.ok,
        generated_at_ms: now_millis(),
        status,
        sessions,
        queued_triggers,
        recent_runs,
        recent_session_inputs,
        event_log,
    }
}

fn build_dashboard_event_log(
    runs: &[TriggerRunRecord],
    session_inputs: &[SessionInputCompletionRecord],
    runtime_tasks: &[GatewayRuntimeTaskStatus],
) -> Vec<GatewayDashboardEventLogEntry> {
    let mut entries = Vec::with_capacity(runs.len() + session_inputs.len() + runtime_tasks.len());
    for run in runs {
        entries.push(GatewayDashboardEventLogEntry {
            kind: "trigger_run".to_string(),
            id: run.id.clone(),
            message: format!("{}: {}", run.status, run.message),
            at_ms: run.ended_at_ms.max(run.started_at_ms),
            session_id: run.session_id.clone(),
        });
    }
    for input in session_inputs {
        entries.push(GatewayDashboardEventLogEntry {
            kind: "session_input_completed".to_string(),
            id: input.input_id.clone(),
            message: input.message_preview.clone(),
            at_ms: input.completed_at_ms.max(input.received_at_ms),
            session_id: Some(input.session_id.clone()),
        });
    }
    for task in runtime_tasks {
        let Some(error) = task.last_error.as_deref() else {
            continue;
        };
        entries.push(GatewayDashboardEventLogEntry {
            kind: "runtime_task_failed".to_string(),
            id: task.name.clone(),
            message: error.to_string(),
            at_ms: task.last_started_at_ms.unwrap_or_default(),
            session_id: None,
        });
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.at_ms));
    entries.truncate(50);
    entries
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
        pending_session_inputs: state.session_input_store.list().len(),
        claimed_triggers,
        dead_letter_runs,
        recent_runs: runs.into_iter().take(20).collect(),
        recent_session_inputs: state.session_input_store.recent_completions(20),
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
        DASHBOARD_HTTP_TASK,
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
        DASHBOARD_HTTP_TASK,
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
                DASHBOARD_HTTP_TASK,
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

fn clean_session_ids(session_ids: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    for session_id in session_ids {
        let session_id = session_id.trim();
        if session_id.is_empty() || cleaned.iter().any(|existing| existing == session_id) {
            continue;
        }
        cleaned.push(session_id.to_string());
    }
    cleaned
}

fn default_session_registry_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".forge")
        .join("gateway-sessions.json")
}

fn load_session_registry(path: &Path) -> HashMap<String, GatewaySessionInfo> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            log::warn!("failed to read gateway session registry: {error}");
            return HashMap::new();
        }
    };
    match serde_json::from_str::<Vec<GatewaySessionInfo>>(&raw) {
        Ok(sessions) => sessions
            .into_iter()
            .filter(|session| !session.session_id.trim().is_empty())
            .map(|mut session| {
                session.restored_from_registry = true;
                (session.session_id.clone(), session)
            })
            .collect(),
        Err(error) => {
            log::warn!("failed to parse gateway session registry: {error}");
            HashMap::new()
        }
    }
}

fn save_session_registry(
    path: &Path,
    sessions: &HashMap<String, GatewaySessionInfo>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("create session dir: {error}"))?;
    }
    let sessions = sorted_sessions(sessions.values().cloned());
    let json = serde_json::to_string_pretty(&sessions)
        .map_err(|error| format!("serialize sessions: {error}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes())
        .map_err(|error| format!("write session registry tmp: {error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("replace session registry: {error}"))?;
    Ok(())
}

fn sorted_sessions(
    sessions: impl IntoIterator<Item = GatewaySessionInfo>,
) -> Vec<GatewaySessionInfo> {
    let mut sessions = sessions.into_iter().collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    sessions
}

fn snapshot_backed_gateway_sessions() -> Vec<GatewaySessionInfo> {
    match crate::agent::snapshot::list_session_snapshots() {
        Ok(snapshots) => snapshots
            .into_iter()
            .map(|snapshot| GatewaySessionInfo {
                session_id: snapshot.session_id,
                provider: snapshot.provider,
                model: snapshot.model,
                workspace_path: snapshot.working_dir,
                created_at_ms: snapshot.created_at_ms,
                owner_pid: None,
                last_seen_at_ms: None,
                restored_from_registry: true,
            })
            .collect(),
        Err(error) => {
            log::warn!("failed to list gateway snapshot-backed sessions: {error}");
            Vec::new()
        }
    }
}

fn active_session_count(sessions: &HashMap<String, GatewaySessionInfo>) -> usize {
    active_session_count_at(sessions, now_millis())
}

fn active_session_count_at(sessions: &HashMap<String, GatewaySessionInfo>, now_ms: u64) -> usize {
    sessions
        .values()
        .filter(|session| session_counts_as_active_at(session, now_ms))
        .count()
}

fn session_counts_as_active_at(session: &GatewaySessionInfo, now_ms: u64) -> bool {
    if session.restored_from_registry {
        return false;
    }

    let Some(last_seen_at_ms) = session.last_seen_at_ms else {
        return true;
    };

    now_ms.saturating_sub(last_seen_at_ms) <= SESSION_STALE_AFTER_MS
}

fn session_attach_status_at(
    session: &GatewaySessionInfo,
    now_ms: u64,
) -> GatewaySessionAttachStatus {
    if session.restored_from_registry {
        return GatewaySessionAttachStatus::Restored;
    }

    if let Some(last_seen_at_ms) = session.last_seen_at_ms {
        if now_ms.saturating_sub(last_seen_at_ms) > SESSION_STALE_AFTER_MS {
            return GatewaySessionAttachStatus::Stale;
        }
    }

    GatewaySessionAttachStatus::Live
}

fn session_attach_message(status: GatewaySessionAttachStatus) -> &'static str {
    match status {
        GatewaySessionAttachStatus::Live => "Session is live and attachable.",
        GatewaySessionAttachStatus::Restored => {
            "Session metadata was restored from the gateway registry; reopen the owning runtime before attaching."
        }
        GatewaySessionAttachStatus::Stale => {
            "Session heartbeat is stale; the owning runtime may have exited unexpectedly."
        }
        GatewaySessionAttachStatus::Missing => "Session is not registered with the gateway.",
    }
}

fn session_attach_control(
    status: GatewaySessionAttachStatus,
    gateway_can_read_snapshot: bool,
) -> GatewaySessionControl {
    match status {
        GatewaySessionAttachStatus::Live => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRuntimeRequired,
            gateway_can_stream: true,
            gateway_can_send_input: true,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action:
                "Queue input through the gateway; the owning desktop runtime will consume it."
                    .to_string(),
        },
        GatewaySessionAttachStatus::Restored => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRestoreRequired,
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action:
                "Restore the session in desktop first; gateway only has registry metadata."
                    .to_string(),
        },
        GatewaySessionAttachStatus::Stale => GatewaySessionControl {
            control_plane: GatewaySessionControlPlane::DesktopRestoreRequired,
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action: "Reopen desktop to recover or clear the stale owner before attaching."
                .to_string(),
        },
        GatewaySessionAttachStatus::Missing => GatewaySessionControl {
            control_plane: if gateway_can_read_snapshot {
                GatewaySessionControlPlane::DesktopRestoreRequired
            } else {
                GatewaySessionControlPlane::Unavailable
            },
            gateway_can_stream: gateway_can_read_snapshot,
            gateway_can_send_input: false,
            gateway_can_resume: false,
            gateway_can_read_snapshot,
            required_action: if gateway_can_read_snapshot {
                "Restore the session from snapshot before attaching.".to_string()
            } else {
                "Register or restore the session before attaching.".to_string()
            },
        },
    }
}

fn gateway_snapshot_summary_for_session(session_id: &str) -> Option<GatewaySessionSnapshotSummary> {
    crate::session_store::get_summary(session_id)
        .ok()
        .flatten()
        .map(|summary| GatewaySessionSnapshotSummary {
            session_id: summary.session_id,
            provider: summary.provider,
            model: summary.model,
            working_dir: summary.working_dir,
            summary: summary.summary,
            created_at_ms: summary.created_at_ms,
            updated_at_ms: summary.updated_at_ms,
            message_count: summary.message_count,
        })
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
    use crate::adapters::base::ChatMessage;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_gateway_state() -> GatewayState {
        GatewayState {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            session_registry_path: None,
            trigger_store: Arc::new(crate::gateway::webhook::TriggerStore::new()),
            trigger_run_store: Arc::new(crate::gateway::runner::TriggerRunStore::new()),
            session_input_store: Arc::new(crate::gateway::session_input::SessionInputStore::new()),
            runtime_tasks: Mutex::new(default_runtime_task_map()),
            include_snapshot_sessions: false,
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
                session_id: None,
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
                session_id: None,
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
                session_id: None,
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
        state
            .session_input_store
            .push(crate::gateway::session_input::SessionInputRecord {
                id: "input-1".into(),
                session_id: "session-1".into(),
                message: "continue".into(),
                received_at_ms: 30,
            });
        state
            .session_input_store
            .complete_with_record("input-1")
            .expect("completion");

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
                assert_eq!(status.pending_session_inputs, 0);
                assert_eq!(status.recent_runs.len(), 2);
                assert_eq!(status.recent_runs[0].id, "run-ok");
                assert_eq!(status.recent_session_inputs.len(), 1);
                assert_eq!(status.recent_session_inputs[0].input_id, "input-1");
                assert_eq!(status.recent_session_inputs[0].session_id, "session-1");
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
                        (DASHBOARD_HTTP_TASK, false),
                    ]
                );
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn dispatch_dashboard_snapshot_returns_dashboard_operational_summary() {
        let state = test_gateway_state();
        state.register_session(test_session("session-1", "claude"));
        state.trigger_store.push(test_trigger("pending-1", None));
        state
            .trigger_store
            .push(test_trigger("claimed-1", Some(1234)));
        state
            .trigger_run_store
            .push(crate::gateway::runner::TriggerRunRecord {
                id: "run-ok".into(),
                trigger_id: "pending-1".into(),
                session_id: Some("session-1".into()),
                attempt: 1,
                status: "completed".into(),
                message: "ok".into(),
                started_at_ms: 20,
                ended_at_ms: 21,
                trigger_message: Some("run digest".into()),
                profile_id: Some("ops".into()),
                provider: Some("claude".into()),
                model: Some("sonnet".into()),
                workspace_path: Some("/repo".into()),
            });
        state
            .session_input_store
            .push(crate::gateway::session_input::SessionInputRecord {
                id: "input-1".into(),
                session_id: "session-1".into(),
                message: "continue".into(),
                received_at_ms: 30,
            });
        state
            .session_input_store
            .complete_with_record("input-1")
            .expect("completion");
        state.mark_runtime_task_failed(WEBHOOK_LISTENER_TASK, "address already in use");

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "dashboard".into(),
                method: "dashboard_snapshot".into(),
                params: None,
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let snapshot: GatewayDashboardSnapshot =
                    serde_json::from_value(resp.result).expect("parse dashboard snapshot");
                assert!(snapshot.ok);
                assert!(snapshot.generated_at_ms > 0);
                assert_eq!(snapshot.status.pending_triggers, 1);
                assert_eq!(snapshot.status.claimed_triggers, 1);
                assert_eq!(snapshot.sessions.len(), 1);
                assert_eq!(snapshot.sessions[0].session_id, "session-1");
                assert_eq!(snapshot.queued_triggers.len(), 2);
                assert_eq!(snapshot.recent_runs.len(), 1);
                assert_eq!(snapshot.recent_runs[0].id, "run-ok");
                assert_eq!(snapshot.recent_session_inputs.len(), 1);
                assert_eq!(snapshot.recent_session_inputs[0].input_id, "input-1");
                assert!(snapshot
                    .event_log
                    .iter()
                    .any(|entry| entry.kind == "trigger_run" && entry.id == "run-ok"));
                assert!(snapshot.event_log.iter().any(|entry| {
                    entry.kind == "session_input_completed" && entry.id == "input-1"
                }));
                assert!(snapshot.event_log.iter().any(|entry| {
                    entry.kind == "runtime_task_failed" && entry.id == WEBHOOK_LISTENER_TASK
                }));
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn dispatch_attach_session_classifies_session_states() {
        let state = test_gateway_state();
        state.register_session(test_session("session-live", "claude"));

        let mut restored = test_session("session-restored", "codex");
        restored.restored_from_registry = true;
        state.register_session(restored);

        let mut stale = test_session("session-stale", "openai");
        stale.last_seen_at_ms = Some(now_millis().saturating_sub(SESSION_STALE_AFTER_MS + 1));
        state.register_session(stale);

        let cases = [
            (
                "session-live",
                crate::gateway::protocol::GatewaySessionAttachStatus::Live,
                true,
            ),
            (
                "session-restored",
                crate::gateway::protocol::GatewaySessionAttachStatus::Restored,
                false,
            ),
            (
                "session-stale",
                crate::gateway::protocol::GatewaySessionAttachStatus::Stale,
                false,
            ),
            (
                "missing",
                crate::gateway::protocol::GatewaySessionAttachStatus::Missing,
                false,
            ),
        ];

        for (session_id, expected_status, expected_ok) in cases {
            let reply = dispatch(
                &state,
                GatewayRequest {
                    id: format!("attach-{session_id}"),
                    method: "attach_session".into(),
                    params: Some(serde_json::json!({ "session_id": format!(" {session_id} ") })),
                },
            );

            match reply {
                GatewayReply::Ok(resp) => {
                    let result: crate::gateway::protocol::AttachSessionResult =
                        serde_json::from_value(resp.result).expect("parse attach result");
                    assert_eq!(result.session_id, session_id);
                    assert_eq!(result.status, expected_status);
                    assert_eq!(result.ok, expected_ok);
                    assert_eq!(
                        result.control.gateway_can_stream,
                        expected_status
                            == crate::gateway::protocol::GatewaySessionAttachStatus::Live
                    );
                    assert_eq!(
                        result.control.gateway_can_send_input,
                        expected_status
                            == crate::gateway::protocol::GatewaySessionAttachStatus::Live
                    );
                    assert!(!result.control.gateway_can_resume);
                    assert!(!result.control.gateway_can_read_snapshot);
                    assert_eq!(
                        result.control.control_plane,
                        match expected_status {
                            crate::gateway::protocol::GatewaySessionAttachStatus::Live =>
                                crate::gateway::protocol::GatewaySessionControlPlane::DesktopRuntimeRequired,
                            crate::gateway::protocol::GatewaySessionAttachStatus::Restored |
                            crate::gateway::protocol::GatewaySessionAttachStatus::Stale =>
                                crate::gateway::protocol::GatewaySessionControlPlane::DesktopRestoreRequired,
                            crate::gateway::protocol::GatewaySessionAttachStatus::Missing =>
                                crate::gateway::protocol::GatewaySessionControlPlane::Unavailable,
                        }
                    );
                    assert_eq!(
                        result.session.is_some(),
                        expected_status
                            != crate::gateway::protocol::GatewaySessionAttachStatus::Missing
                    );
                }
                _ => panic!("expected Ok attach reply for {session_id}"),
            }
        }
    }

    #[test]
    fn session_attach_control_reports_readable_snapshot_capability() {
        let control = session_attach_control(
            crate::gateway::protocol::GatewaySessionAttachStatus::Live,
            true,
        );

        assert_eq!(
            control.control_plane,
            crate::gateway::protocol::GatewaySessionControlPlane::DesktopRuntimeRequired
        );
        assert!(control.gateway_can_stream);
        assert!(control.gateway_can_send_input);
        assert!(!control.gateway_can_resume);
        assert!(control.gateway_can_read_snapshot);
    }

    #[test]
    fn session_attach_control_routes_missing_snapshot_to_restore_action() {
        let control = session_attach_control(
            crate::gateway::protocol::GatewaySessionAttachStatus::Missing,
            true,
        );

        assert_eq!(
            control.control_plane,
            crate::gateway::protocol::GatewaySessionControlPlane::DesktopRestoreRequired
        );
        assert!(control.gateway_can_stream);
        assert!(control.gateway_can_read_snapshot);
        assert!(control.required_action.contains("snapshot"));
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
    fn default_runtime_tasks_include_dashboard_http() {
        let task_names = default_runtime_task_statuses()
            .into_iter()
            .map(|task| task.name)
            .collect::<Vec<_>>();

        assert!(task_names.contains(&DASHBOARD_HTTP_TASK.to_string()));
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
    fn dispatch_enqueue_session_input_pushes_to_inbox_and_updates_runtime_status() {
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "enqueue-input".into(),
                method: "enqueue_session_input".into(),
                params: Some(serde_json::json!({
                    "input_id": "input-ipc-1",
                    "session_id": " session-1 ",
                    "message": " continue the work "
                })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::EnqueueSessionInputResult =
                    serde_json::from_value(resp.result).expect("parse enqueue input result");
                assert!(result.ok);
                assert_eq!(result.input_id, "input-ipc-1");
                assert_eq!(result.session_id, "session-1");
                assert_eq!(result.pending_inputs, 1);
            }
            _ => panic!("expected Ok reply"),
        }

        let queued = state.session_input_store.list();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, "input-ipc-1");
        assert_eq!(queued[0].session_id, "session-1");
        assert_eq!(queued[0].message, "continue the work");

        let status = build_runtime_status(&state);
        assert_eq!(status.pending_session_inputs, 1);
    }

    #[test]
    fn dispatch_enqueue_session_input_rejects_blank_message() {
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "enqueue-input".into(),
                method: "enqueue_session_input".into(),
                params: Some(serde_json::json!({
                    "session_id": "session-1",
                    "message": "   "
                })),
            },
        );

        match reply {
            GatewayReply::Err(err) => {
                assert_eq!(err.error.code, -32602);
                assert!(err.error.message.contains("message"));
            }
            _ => panic!("expected Err reply"),
        }
        assert!(state.session_input_store.list().is_empty());
    }

    #[test]
    fn dispatch_list_session_inputs_filters_live_session_ids() {
        let state = test_gateway_state();
        state
            .session_input_store
            .push(crate::gateway::session_input::SessionInputRecord {
                id: "input-2".into(),
                session_id: "session-2".into(),
                message: "skip".into(),
                received_at_ms: 20,
            });
        state
            .session_input_store
            .push(crate::gateway::session_input::SessionInputRecord {
                id: "input-1".into(),
                session_id: "session-1".into(),
                message: "continue".into(),
                received_at_ms: 10,
            });

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "list-inputs".into(),
                method: "list_session_inputs".into(),
                params: Some(serde_json::json!({
                    "session_ids": [" session-1 ", "session-1"],
                    "limit": 10
                })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::ListSessionInputsResult =
                    serde_json::from_value(resp.result).expect("parse list input result");
                assert!(result.ok);
                assert_eq!(result.pending_inputs, 2);
                assert_eq!(result.inputs.len(), 1);
                assert_eq!(result.inputs[0].id, "input-1");
            }
            _ => panic!("expected Ok reply"),
        }
    }

    #[test]
    fn dispatch_complete_session_input_removes_record() {
        let state = test_gateway_state();
        state
            .session_input_store
            .push(crate::gateway::session_input::SessionInputRecord {
                id: "input-1".into(),
                session_id: "session-1".into(),
                message: "continue".into(),
                received_at_ms: 10,
            });

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "complete-input".into(),
                method: "complete_session_input".into(),
                params: Some(serde_json::json!({
                    "input_id": " input-1 "
                })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::CompleteSessionInputResult =
                    serde_json::from_value(resp.result).expect("parse complete input result");
                assert!(result.ok);
                assert!(result.removed);
                assert_eq!(result.input_id, "input-1");
                assert_eq!(result.pending_inputs, 0);
            }
            _ => panic!("expected Ok reply"),
        }
        assert!(state.session_input_store.list().is_empty());
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
                session_id: None,
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
                session_id: None,
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
                session_id: None,
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

    #[test]
    fn dispatch_get_session_snapshot_returns_saved_snapshot_detail() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        std::env::set_var("HOME", home.path());
        let snapshot = crate::agent::snapshot::AgentSessionSnapshot::new(
            "snapshot-detail-session".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "/repo/detail".to_string(),
            vec![ChatMessage::user("show me".into())],
            Some("detail summary".to_string()),
            Some(128_000),
        );
        crate::agent::snapshot::save_session_snapshot(&snapshot).expect("save snapshot");
        let state = GatewayState::new();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "snapshot-detail".into(),
                method: "get_session_snapshot".into(),
                params: Some(serde_json::json!({"session_id": " snapshot-detail-session "})),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::GetSessionSnapshotResult =
                    serde_json::from_value(resp.result).expect("parse snapshot result");
                assert!(result.ok);
                assert_eq!(result.session_id, "snapshot-detail-session");
                assert_eq!(result.snapshot["session_id"], "snapshot-detail-session");
                assert_eq!(result.snapshot["provider"], "deepseek");
                assert_eq!(result.snapshot["messages"][0]["content"], "show me");
            }
            _ => panic!("expected Ok reply"),
        }

        if let Some(value) = previous_home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn dispatch_tail_session_events_returns_transcript_cursor_window() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        std::env::set_var("HOME", home.path());
        crate::transcript::append_transcript_event(serde_json::json!({
            "event_type": "user_message",
            "session_id": "tail-session",
            "block_id": "user-1",
            "content": "hello"
        }))
        .expect("append first event");
        crate::transcript::append_transcript_event(serde_json::json!({
            "event_type": "text_chunk",
            "session_id": "tail-session",
            "block_id": "text-1",
            "content": "world"
        }))
        .expect("append second event");
        let state = test_gateway_state();

        let reply = dispatch(
            &state,
            GatewayRequest {
                id: "tail-events".into(),
                method: "tail_session_events".into(),
                params: Some(serde_json::json!({
                    "session_id": " tail-session ",
                    "after_cursor": 1,
                    "limit": 10
                })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::TailSessionEventsResult =
                    serde_json::from_value(resp.result).expect("parse tail result");
                assert!(result.ok);
                assert_eq!(result.session_id, "tail-session");
                assert_eq!(result.events.len(), 1);
                assert_eq!(result.events[0]["event_type"], "text_chunk");
                assert_eq!(result.total_events, 2);
                assert_eq!(result.next_cursor, 2);
                assert!(!result.cursor_reset);
            }
            _ => panic!("expected Ok reply"),
        }

        if let Some(value) = previous_home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }

    // ── GatewayState ────────────────────────────────────────────────────

    #[test]
    fn gateway_state_starts_with_zero_sessions() {
        let state = test_gateway_state();
        assert_eq!(state.active_sessions(), 0);
    }

    #[test]
    fn gateway_state_registers_and_unregisters_session_count() {
        let state = test_gateway_state();
        state.register_session(test_session("session-1", "claude"));
        assert_eq!(state.active_sessions(), 1);
        assert_eq!(state.list_sessions().len(), 1);

        state.unregister_session("session-1");
        assert_eq!(state.active_sessions(), 0);
        assert!(state.list_sessions().is_empty());
    }

    #[test]
    fn gateway_state_replacing_session_does_not_double_count() {
        let state = test_gateway_state();
        state.register_session(test_session("session-1", "claude"));
        state.register_session(test_session("session-1", "codex"));

        let sessions = state.list_sessions();
        assert_eq!(state.active_sessions(), 1);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider, "codex");
    }

    #[test]
    fn gateway_state_lists_snapshot_only_sessions_as_restored() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        std::env::set_var("HOME", home.path());
        let snapshot = crate::agent::snapshot::AgentSessionSnapshot::new(
            "snapshot-only-session".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "/repo/snapshot".to_string(),
            vec![ChatMessage::user("hello".into())],
            Some("snapshot summary".to_string()),
            Some(128_000),
        );
        crate::agent::snapshot::save_session_snapshot(&snapshot).expect("save snapshot");
        let state = GatewayState::new();

        let sessions = state.list_sessions();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "snapshot-only-session");
        assert_eq!(sessions[0].provider, "deepseek");
        assert_eq!(sessions[0].model, "deepseek-v4-flash");
        assert_eq!(sessions[0].workspace_path, "/repo/snapshot");
        assert!(sessions[0].restored_from_registry);
        assert_eq!(state.active_sessions(), 0);

        if let Some(value) = previous_home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn gateway_state_unregistering_missing_session_keeps_count_at_zero() {
        let state = test_gateway_state();
        state.unregister_session("missing-session");

        assert_eq!(state.active_sessions(), 0);
    }

    #[test]
    fn active_session_count_excludes_stale_live_sessions() {
        let now_ms = SESSION_STALE_AFTER_MS + 10;
        let mut fresh = test_session("fresh-session", "claude");
        fresh.last_seen_at_ms = Some(now_ms);
        let mut stale = test_session("stale-session", "codex");
        stale.last_seen_at_ms = Some(1);
        let mut restored = test_session("restored-session", "openai");
        restored.last_seen_at_ms = Some(now_ms);
        restored.restored_from_registry = true;

        let sessions = HashMap::from([
            (fresh.session_id.clone(), fresh),
            (stale.session_id.clone(), stale),
            (restored.session_id.clone(), restored),
        ]);

        assert_eq!(active_session_count_at(&sessions, now_ms), 1);
    }

    #[test]
    fn gateway_session_registry_restores_sessions_without_marking_them_active() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("gateway-sessions.json");
        let state = GatewayState::new_with_session_registry_path(registry_path.clone());
        state.register_session(test_session("session-1", "claude"));

        let restored = GatewayState::new_with_session_registry_path(registry_path.clone());
        let sessions = restored.list_sessions();

        assert_eq!(restored.active_sessions(), 0);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-1");
        assert_eq!(sessions[0].provider, "claude");
        assert!(sessions[0].restored_from_registry);

        restored.register_session(test_session("session-1", "claude"));
        let live_sessions = restored.list_sessions();
        assert_eq!(restored.active_sessions(), 1);
        assert!(!live_sessions[0].restored_from_registry);
    }

    #[test]
    fn gateway_session_registry_persists_unregistered_sessions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("gateway-sessions.json");
        let state = GatewayState::new_with_session_registry_path(registry_path.clone());
        state.register_session(test_session("session-1", "claude"));
        state.unregister_session("session-1");

        let restored = GatewayState::new_with_session_registry_path(registry_path);

        assert_eq!(restored.active_sessions(), 0);
        assert!(restored.list_sessions().is_empty());
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
            owner_pid: Some(42),
            last_seen_at_ms: Some(now_millis()),
            restored_from_registry: false,
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
