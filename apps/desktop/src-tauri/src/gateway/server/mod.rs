//! Gateway server — Unix-domain-socket listener, request dispatch, and
//! response serialization.

use crate::gateway::protocol::{
    default_gateway_degraded_mode_status, default_gateway_degraded_recovery_command,
    default_gateway_ownership_capability, serialize_reply, AttachSessionParams,
    AttachSessionResult, CancelLoopTaskParams, CancelLoopTaskResult, CancelTriggerParams,
    CancelTriggerResult, ClearStaleSessionInputParams, ClearStaleSessionInputResult,
    CompleteSessionInputParams, CompleteSessionInputResult, CreateLoopTaskRequest,
    EnqueueSessionInputParams, EnqueueSessionInputResult, EnqueueTriggerParams,
    EnqueueTriggerResult, EvaluateLoopTaskCompletionParams, EvaluateLoopTaskCompletionResult,
    GatewayDegradedModeStatus, GatewayError, GatewayErrorBody, GatewayOwnershipCapability,
    GatewayOwnershipEligibilityDecision, GatewayOwnershipEligibilityParams,
    GatewayOwnershipEligibilityResult, GatewayOwnershipMode, GatewayReadOnlyOwnerDiagnosticsParams,
    GatewayReadOnlyOwnerDiagnosticsResult, GatewayReadOnlyOwnerSideEffects, GatewayReply,
    GatewayRequest, GatewayResponse, GatewaySessionAttachStatus, GatewaySessionControl,
    GatewaySessionControlPlane, GatewaySessionInfo, GatewaySessionSnapshotSummary,
    GetLoopTaskParams, GetSessionSnapshotParams, GetSessionSnapshotResult, GetTriggerRunParams,
    GetTriggerRunResult, HeadlessResumeControlParams, HeadlessResumeControlResult, HealthResult,
    ListLoopTasksParams, ListLoopTasksResult, ListSessionInputsParams, ListSessionInputsResult,
    LoopTaskResponse, PingResult, RecoverLoopTaskParams, RecoverLoopTaskResult,
    RecoveryActionEvidence, RecoveryActionKind, ReplayTriggerRunParams, ReplayTriggerRunResult,
    TailSessionEventsParams, TailSessionEventsResult, GATEWAY_VERSION,
};
use crate::gateway::runner::{TriggerRunRecord, TriggerRunStore};
use crate::gateway::session_input::{
    new_session_input_record, SessionInputCompletionRecord, SessionInputStore,
};
use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use crate::loop_runtime::runner::{LoopRunnerQueueStats, LoopTaskRunner};
use crate::loop_runtime::types::new_loop_event_id;
use crate::loop_runtime::{
    default_runtime_health_snapshot, evaluate_completion, HeadlessOwnerExecutorKind,
    HeadlessOwnerRun, HeadlessOwnerRunState, HeadlessOwnerSnapshotSource, HeadlessResumeApproval,
    HeadlessResumeMode, LoopActor, LoopEventEnvelope, LoopEventJournal, LoopRuntimeEvent,
    LoopTaskProjectionStore, LoopTaskRecord, LoopTaskStatus, RuntimeHealthSnapshot,
    RuntimeHealthSnapshotInput, RuntimeObservedTask, RuntimeReplayHealth,
    LOOP_RUNTIME_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

mod loop_tasks;
mod ownership;
mod session_inputs;
mod sessions;
mod status;
mod triggers;

use loop_tasks::*;
use ownership::*;
use session_inputs::*;
use sessions::*;
use status::*;
use triggers::*;

#[cfg(test)]
mod tests;

/// Gateway runtime snapshot for diagnostics/status surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayRuntimeStatus {
    pub ok: bool,
    pub message: String,
    #[serde(default = "default_gateway_ownership_capability")]
    pub ownership: GatewayOwnershipCapability,
    #[serde(default = "default_gateway_degraded_mode_status")]
    pub degraded_mode: GatewayDegradedModeStatus,
    #[serde(default = "default_runtime_health_snapshot")]
    pub runtime_health: RuntimeHealthSnapshot,
    pub uptime_seconds: u64,
    pub active_sessions: usize,
    pub pending_triggers: usize,
    #[serde(default)]
    pub pending_session_inputs: usize,
    #[serde(default)]
    pub loop_runner: String,
    #[serde(default)]
    pub pending_loop_tasks: usize,
    #[serde(default)]
    pub running_loop_tasks: usize,
    #[serde(default)]
    pub stale_loop_task_leases: usize,
    #[serde(default)]
    pub orphaned_loop_tasks: usize,
    #[serde(default)]
    pub interrupted_loop_tasks: usize,
    #[serde(default)]
    pub recoverable_loop_tasks: usize,
    #[serde(default)]
    pub dry_run_headless_owner_runs: usize,
    #[serde(default)]
    pub waiting_headless_owner_runs: usize,
    #[serde(default)]
    pub denied_headless_owner_runs: usize,
    #[serde(default)]
    pub expired_headless_owner_runs: usize,
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
    #[serde(default)]
    pub loop_tasks: Vec<LoopTaskRecord>,
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
    pub loop_event_journal: Arc<LoopEventJournal>,
    pub loop_task_projection_store: Arc<LoopTaskProjectionStore>,
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
            loop_event_journal: Arc::new(LoopEventJournal::persistent_default()),
            loop_task_projection_store: Arc::new(LoopTaskProjectionStore::persistent_default()),
            runtime_tasks: Mutex::new(default_runtime_task_map()),
            include_snapshot_sessions,
        }
    }

    pub fn new_with_loop_runtime_stores(
        loop_event_journal: Arc<LoopEventJournal>,
        loop_task_projection_store: Arc<LoopTaskProjectionStore>,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: Mutex::new(HashMap::new()),
            session_registry_path: None,
            trigger_store: Arc::new(TriggerStore::new()),
            trigger_run_store: Arc::new(TriggerRunStore::new()),
            session_input_store: Arc::new(SessionInputStore::new()),
            loop_event_journal,
            loop_task_projection_store,
            runtime_tasks: Mutex::new(default_runtime_task_map()),
            include_snapshot_sessions: false,
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

pub const LOOP_RUNNER_TASK: &str = "loop_runner";

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
        "clear_stale_session_input" => handle_clear_stale_session_input(state, request),
        "cancel_trigger" => handle_cancel_trigger(state, request),
        "replay_trigger_run" => handle_replay_trigger_run(state, request),
        "get_trigger_run" => handle_get_trigger_run(state, request),
        "get_session_snapshot" => handle_get_session_snapshot(request),
        "tail_session_events" => handle_tail_session_events(request),
        "create_loop_task" => handle_create_loop_task(state, request),
        "list_loop_tasks" => handle_list_loop_tasks(state, request),
        "get_loop_task" => handle_get_loop_task(state, request),
        "request_headless_resume" => handle_request_headless_resume(state, request),
        "run_gateway_read_only_owner_diagnostics" => {
            handle_run_gateway_read_only_owner_diagnostics(state, request)
        }
        "evaluate_gateway_ownership_eligibility" => {
            handle_evaluate_gateway_ownership_eligibility(state, request)
        }
        "evaluate_loop_task_completion" => handle_evaluate_loop_task_completion(state, request),
        "cancel_loop_task" => handle_cancel_loop_task(state, request),
        "recover_loop_task" => handle_recover_loop_task(state, request),
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

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
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

fn stable_text_fingerprint(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
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
