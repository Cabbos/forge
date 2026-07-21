use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::adapters::build_adapter_with_profiles;
use crate::agent::provider_capabilities::{missing_api_key_message, normalize_provider};
use crate::agent::session::{AgentSession, SessionStatus};
use crate::agent::session_events;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::session_journal::{
    JournalDamage, JournalError, JournalLoadResult, SessionJournalStore, SessionMutationEnvelope,
};
use crate::agent::session_projection::SessionProjection;
use crate::agent::snapshot::{
    delete_session_snapshot, list_session_snapshots, save_session_snapshot,
    try_load_session_snapshot, ActiveToolCallDescriptor, AgentSessionSnapshot,
    PendingConfirmDescriptor, SnapshotLoadFailure,
};
use crate::gateway::protocol::GatewaySessionInfo;
use crate::harness::Harness;
use crate::ipc::delivery_summary::emit_delivery_summary;
use crate::ipc::session_builder::{build_agent_session, BuildAgentSessionRequest};
use crate::protocol::commands::{SessionCreated, SessionInfo};
use crate::protocol::events::{DeliverySummary, StreamEvent};
use crate::protocol::BlockId;
use crate::settings;
use crate::state::AppState;
use crate::workflow::WorkflowState;
use crate::workspace_safety::resolve_workspace_path as resolve_safe_workspace_path;
use tauri::Manager;

const GATEWAY_SESSION_HEARTBEAT_INTERVAL_SECS: u64 = 60;

pub(crate) fn emit_missing_api_key_notice(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    provider: &str,
) {
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::Error {
            session_id: session_id.to_string(),
            block_id: BlockId::new().to_string(),
            message: missing_api_key_message(provider),
            code: "missing_api_key".to_string(),
        },
    );
}

pub(crate) fn emit_session_started(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    provider: &str,
    model: &str,
    context_window_tokens: Option<u32>,
) {
    crate::log_store::log_event(
        "INFO",
        "session",
        &format!("session '{session_id}' created (provider={provider}, model={model})"),
        Some(session_id),
    );
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::SessionStarted {
            session_id: session_id.to_string(),
            agent_type: provider.to_string(),
            model: model.to_string(),
            context_window_tokens,
        },
    );
}

pub(crate) async fn emit_session_projection_and_delivery(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    session: &AgentSession,
) {
    if let Some(workflow) = state.workflow_states.read().await.get(session_id).cloned() {
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::WorkflowUpdated {
                session_id: session_id.to_string(),
                state: workflow,
            },
        );
    }
    session.emit_latest_turn_projection(app_handle);
    if let Some(delivery) = state.delivery_states.read().await.get(session_id).cloned() {
        emit_delivery_summary(app_handle, session_id, delivery);
    }
}

pub(crate) async fn register_and_dispatch_session_start(
    state: &Arc<AppState>,
    session: Arc<AgentSession>,
    session_id: &str,
) {
    state
        .register_session(session_id.to_string(), session.clone())
        .await;
    register_gateway_session_best_effort(session_id, &session).await;
    let _ = session
        .harness
        .dispatch_session_start_event(session_id)
        .await;
}

pub(crate) fn gateway_session_info_for_session(
    session_id: &str,
    session: &AgentSession,
) -> GatewaySessionInfo {
    let snapshot = session.snapshot();
    GatewaySessionInfo {
        session_id: session_id.to_string(),
        provider: session.agent_type.clone(),
        model: session.model_id.clone(),
        workspace_path: snapshot.working_dir,
        created_at_ms: snapshot.created_at_ms,
        owner_pid: Some(std::process::id()),
        last_seen_at_ms: Some(now_millis()),
        restored_from_registry: false,
    }
}

pub(crate) async fn register_gateway_session_best_effort(session_id: &str, session: &AgentSession) {
    let info = gateway_session_info_for_session(session_id, session);
    if let Err(error) = crate::gateway::client::try_register_session(info).await {
        crate::app_log!(
            "WARN",
            "[gateway] failed to register session '{session_id}': {error}"
        );
    }
}

pub(crate) async fn unregister_gateway_session_best_effort(session_id: &str) {
    if let Err(error) = crate::gateway::client::try_unregister_session(session_id).await {
        crate::app_log!(
            "WARN",
            "[gateway] failed to unregister session '{session_id}': {error}"
        );
    }
}

pub(crate) async fn gateway_session_ids_for_shutdown(state: &Arc<AppState>) -> Vec<String> {
    let mut ids = state
        .sessions
        .read()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

pub(crate) async fn gateway_session_infos_for_state(
    state: &Arc<AppState>,
) -> Vec<GatewaySessionInfo> {
    let sessions = state.sessions.read().await;
    let mut infos = sessions
        .iter()
        .map(|(session_id, session)| gateway_session_info_for_session(session_id, session))
        .collect::<Vec<_>>();
    infos.sort_by(|left, right| left.session_id.cmp(&right.session_id));
    infos
}

pub(crate) async fn heartbeat_gateway_sessions_once(state: &Arc<AppState>) {
    for info in gateway_session_infos_for_state(state).await {
        let session_id = info.session_id.clone();
        if let Err(error) = crate::gateway::client::try_register_session(info).await {
            crate::app_log!(
                "WARN",
                "[gateway] failed to heartbeat session '{session_id}': {error}"
            );
        }
    }
}

pub(crate) fn spawn_gateway_session_heartbeat(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(GATEWAY_SESSION_HEARTBEAT_INTERVAL_SECS)).await;
            let Some(state) = app_handle.try_state::<Arc<AppState>>() else {
                break;
            };
            let state = state.inner().clone();
            heartbeat_gateway_sessions_once(&state).await;
        }
    });
}

pub(crate) async fn unregister_all_gateway_sessions_best_effort(state: &Arc<AppState>) {
    let session_ids = gateway_session_ids_for_shutdown(state).await;
    if session_ids.is_empty() {
        return;
    }
    crate::app_log!(
        "INFO",
        "[gateway] shutdown unregister: {} live session(s)",
        session_ids.len()
    );
    for session_id in session_ids {
        unregister_gateway_session_best_effort(&session_id).await;
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Journal-backed restore selection ────────────────────────────────────────
//
// The selector below is a PURE function: all file reads happen in
// `restore_session_from_snapshot` / the diagnostics scan, and all UI emission
// happens in `emit_restored_session_startup`. This keeps the full
// snapshot-x-journal decision matrix unit-testable.
//
// "Snapshot behind" detection rule
// --------------------------------
// Snapshots written before the mutation journal existed carry
// `journal_sequence == 0` (legacy), and Task 4 does not write sequence
// metadata into snapshots either — but those sessions get a baseline journal
// (SessionInitialized + ConversationReplaced + RuntimeStateUpdated) recorded
// at restore time, so a legacy snapshot is CONTENT-equal to its journal, not
// behind it. The rule is therefore:
//
//   behind  <=>  snapshot.journal_sequence > 0
//                && journal.last_sequence > snapshot.journal_sequence
//
// A legacy snapshot (sequence 0) is never "behind": the snapshot wins and a
// parity diagnostic compares content instead (Healthy vs Diverged). A
// snapshot is only superseded by its journal when the snapshot itself
// recorded which sequence it captured and the journal has since moved on.

/// Journal load input for the restore selector: the replayable event prefix
/// plus any tolerated damage (a torn final line).
#[derive(Debug)]
pub(crate) struct SessionJournalLoad {
    pub events: Vec<SessionMutationEnvelope>,
    pub damage: Option<JournalDamage>,
}

impl From<JournalLoadResult> for SessionJournalLoad {
    fn from(result: JournalLoadResult) -> Self {
        Self {
            events: result.events,
            damage: result.damage,
        }
    }
}

/// What the restore selector decided to restore from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionRestoreSource {
    /// The on-disk snapshot is authoritative.
    Snapshot,
    /// The journal projection is authoritative; restored through the same
    /// in-memory restore path via `SessionProjection::to_snapshot()`.
    JournalProjection,
    /// Nothing durable to restore — start a fresh session.
    Fresh,
}

/// Per-session parity classification consumed by the diagnostics summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionParityStatus {
    /// Snapshot and journal agree (by sequence, or by content for legacy
    /// snapshots).
    Healthy,
    /// Same recorded sequence but content differs. Today this only happens
    /// through the known `repair_message_history` parity gap (it mutates
    /// already-corrupt history outside the journaled helpers), so it is a
    /// soft signal, never a failure.
    Diverged,
    /// The journal has committed events beyond the snapshot's recorded
    /// sequence; the journal projection is newer.
    SnapshotBehind,
    /// Only a snapshot exists (no journal events).
    SnapshotOnly,
    /// Only a usable journal exists (snapshot missing or corrupt).
    JournalOnly,
    /// The journal had a torn final line; the valid prefix was used.
    TornFinalLine,
    /// The journal had a corrupt interior line (or unreplayable events) and
    /// was quarantined.
    CorruptInterior,
}

/// User-visible outcome of a restore decision, surfaced through the existing
/// `StreamEvent::RecoveryNotice` transport — no new IPC surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionRestoreNotice {
    /// The snapshot was corrupt/unreadable; the journal projection was used.
    SnapshotRecoveredFromJournal,
    /// The journal ended in a torn write; the valid prefix was used.
    JournalTornFinalLine,
    /// The journal had a corrupt interior line and was quarantined (renamed
    /// aside; journaling restarts on a fresh generation).
    JournalQuarantined,
    /// Neither snapshot nor journal could be restored; starting fresh.
    RestoreFailedFreshStart,
}

/// Output of `choose_session_restore_source`.
#[derive(Debug)]
pub(crate) struct SessionRestoreDecision {
    pub source: SessionRestoreSource,
    pub parity: SessionParityStatus,
    /// Snapshot-shaped payload to restore through the existing restore path;
    /// `None` means start fresh.
    pub snapshot: Option<AgentSessionSnapshot>,
    pub notices: Vec<SessionRestoreNotice>,
}

/// Pure restore-source selector. See the module-level rule documentation for
/// the "snapshot behind" semantics. All combinations of the snapshot x
/// journal matrix are decided here; I/O and UI stay outside.
pub(crate) fn choose_session_restore_source(
    snapshot: Result<Option<AgentSessionSnapshot>, SnapshotLoadFailure>,
    journal: Result<Option<SessionJournalLoad>, JournalError>,
) -> SessionRestoreDecision {
    // Classify the journal side first.
    enum JournalState {
        Absent,
        /// Load error that leaves the journal's contents unknowable but does
        /// not prove corruption (I/O failures, lock poisoning, schema or
        /// sequence errors surfaced before any event replays). Never restored
        /// from, but not quarantined.
        Unusable,
        /// Proven on-disk corruption (corrupt interior line, or events that
        /// cannot form a projection). Quarantined so journaling restarts.
        Corrupt,
        /// Replayed successfully; possibly with a torn final line. Boxed to
        /// keep the local enum small.
        Usable {
            projection: Box<SessionProjection>,
            torn: bool,
        },
    }
    let journal_state = match journal {
        Ok(None) => JournalState::Absent,
        Ok(Some(load)) => match SessionProjection::from_events(&load.events) {
            Ok(projection) => JournalState::Usable {
                projection: Box::new(projection),
                torn: matches!(load.damage, Some(JournalDamage::TornFinalLine { .. })),
            },
            Err(_) => JournalState::Corrupt,
        },
        Err(JournalError::CorruptInteriorLine { .. }) => JournalState::Corrupt,
        Err(_) => JournalState::Unusable,
    };

    let decision = |source, parity, snapshot, notices| SessionRestoreDecision {
        source,
        parity,
        snapshot,
        notices,
    };

    match (snapshot, journal_state) {
        (Ok(Some(snapshot)), JournalState::Usable { projection, torn }) => {
            let behind = snapshot.journal_sequence > 0
                && projection.last_sequence > snapshot.journal_sequence;
            if behind {
                let payload = journal_projection_payload(&projection);
                let (parity, notices) = if torn {
                    (
                        SessionParityStatus::TornFinalLine,
                        vec![SessionRestoreNotice::JournalTornFinalLine],
                    )
                } else {
                    (SessionParityStatus::SnapshotBehind, Vec::new())
                };
                decision(
                    SessionRestoreSource::JournalProjection,
                    parity,
                    Some(payload),
                    notices,
                )
            } else if torn {
                decision(
                    SessionRestoreSource::Snapshot,
                    SessionParityStatus::TornFinalLine,
                    Some(snapshot),
                    vec![SessionRestoreNotice::JournalTornFinalLine],
                )
            } else {
                let parity =
                    if snapshots_restore_parity_equivalent(&snapshot, &projection.to_snapshot()) {
                        SessionParityStatus::Healthy
                    } else {
                        SessionParityStatus::Diverged
                    };
                decision(
                    SessionRestoreSource::Snapshot,
                    parity,
                    Some(snapshot),
                    Vec::new(),
                )
            }
        }
        (Ok(Some(snapshot)), JournalState::Corrupt) => decision(
            SessionRestoreSource::Snapshot,
            SessionParityStatus::CorruptInterior,
            Some(snapshot),
            vec![SessionRestoreNotice::JournalQuarantined],
        ),
        (Ok(Some(snapshot)), JournalState::Unusable | JournalState::Absent) => decision(
            SessionRestoreSource::Snapshot,
            SessionParityStatus::SnapshotOnly,
            Some(snapshot),
            Vec::new(),
        ),
        (Ok(None), JournalState::Usable { projection, torn }) => {
            let (parity, notices) = if torn {
                (
                    SessionParityStatus::TornFinalLine,
                    vec![SessionRestoreNotice::JournalTornFinalLine],
                )
            } else {
                (SessionParityStatus::JournalOnly, Vec::new())
            };
            decision(
                SessionRestoreSource::JournalProjection,
                parity,
                Some(journal_projection_payload(&projection)),
                notices,
            )
        }
        (Err(_), JournalState::Usable { projection, torn }) => {
            let mut notices = vec![SessionRestoreNotice::SnapshotRecoveredFromJournal];
            let parity = if torn {
                notices.push(SessionRestoreNotice::JournalTornFinalLine);
                SessionParityStatus::TornFinalLine
            } else {
                SessionParityStatus::JournalOnly
            };
            decision(
                SessionRestoreSource::JournalProjection,
                parity,
                Some(journal_projection_payload(&projection)),
                notices,
            )
        }
        (_snapshot_outcome, JournalState::Corrupt) => decision(
            SessionRestoreSource::Fresh,
            SessionParityStatus::CorruptInterior,
            None,
            // Whether the snapshot was missing or corrupt, the user had
            // journal data that can no longer be replayed: quarantine it and
            // start fresh with a recovery notice.
            vec![
                SessionRestoreNotice::JournalQuarantined,
                SessionRestoreNotice::RestoreFailedFreshStart,
            ],
        ),
        (snapshot_outcome, JournalState::Unusable | JournalState::Absent) => {
            let had_durable_data = snapshot_outcome.is_err();
            decision(
                SessionRestoreSource::Fresh,
                SessionParityStatus::SnapshotOnly,
                None,
                if had_durable_data {
                    vec![SessionRestoreNotice::RestoreFailedFreshStart]
                } else {
                    Vec::new()
                },
            )
        }
    }
}

/// Snapshot-shaped restore payload for a journal projection. The journal
/// sequence is stamped so the next snapshot save and restore cycle can apply
/// the "snapshot behind" rule; the generation label is filled in by the save
/// path, which knows the active journal file.
fn journal_projection_payload(projection: &SessionProjection) -> AgentSessionSnapshot {
    let mut snapshot = projection.to_snapshot();
    snapshot.journal_sequence = projection.last_sequence;
    snapshot
}

/// Session-restore parity comparator.
///
/// INVARIANT: this is a POSITIVE ALLOWLIST — it compares only the fields the
/// mutation journal captures. Adding a field to `AgentSessionSnapshot` can
/// never silently break parity (new fields are simply not compared until a
/// journaled counterpart exists); conversely, when a field IS added to the
/// journaled runtime state (`SessionRuntimeState`), it must be added here
/// too, or parity will not cover it.
///
/// Intentionally not compared:
/// - `latest_workflow`, `latest_delivery`, `pending_confirms`,
///   `active_tool_calls`: owned by AppState/IPC layers and never journaled
///   (see agent/session_mutation.rs "Deferred");
/// - `context_window_tokens`: not captured by the journal;
/// - `created_at_ms`/`updated_at_ms`, `schema_version`, `journal_generation`,
///   `journal_sequence`: bookkeeping metadata, not session content. Nested
///   timestamps inside journaled runtime state (turn/goal/A2A) round-trip
///   unchanged and ARE compared exactly, matching `snapshots_parity_equivalent`.
fn snapshots_restore_parity_equivalent(
    left: &AgentSessionSnapshot,
    right: &AgentSessionSnapshot,
) -> bool {
    fn comparable(snapshot: &AgentSessionSnapshot) -> serde_json::Value {
        serde_json::json!({
            "session_id": snapshot.session_id,
            "provider": snapshot.provider,
            "model": snapshot.model,
            "working_dir": snapshot.working_dir,
            "messages": snapshot.messages,
            "summary": snapshot.summary,
            "latest_turn": snapshot.latest_turn,
            "goal_ledger": snapshot.goal_ledger,
            "a2a_state": snapshot.a2a_state,
        })
    }
    comparable(left) == comparable(right)
}

/// Render a restore notice through the existing recovery-notice surface.
fn restore_notice_render(
    notice: SessionRestoreNotice,
) -> (&'static str, &'static str, &'static str, bool) {
    match notice {
        SessionRestoreNotice::SnapshotRecoveredFromJournal => (
            "Session recovered from durable history",
            "The saved session snapshot was unreadable, so Forge restored your conversation from the durable session journal.",
            "snapshot_recovered_from_journal",
            true,
        ),
        SessionRestoreNotice::JournalTornFinalLine => (
            "Session history repaired",
            "The end of the durable session journal was incomplete after an interrupted write; Forge restored the complete prefix.",
            "journal_torn_final_line",
            true,
        ),
        SessionRestoreNotice::JournalQuarantined => (
            "Session journal quarantined",
            "The durable session journal contained corrupted data and was set aside; the session was restored from its snapshot and journaling restarted fresh.",
            "journal_quarantined",
            true,
        ),
        SessionRestoreNotice::RestoreFailedFreshStart => (
            "Session restore failed",
            "Forge could not restore your last session and started fresh. Your data is safe.",
            "snapshot_restore_failed",
            false,
        ),
    }
}

/// Build recovery-notice stream events for restore notices. Shared by the
/// success path (`emit_restored_session_startup`) and the failure/fresh path
/// (`startup_restore_active_session`, `resume_session`) so Fresh outcomes —
/// which have no session to attach notices to — still reach the UI through
/// the existing transport.
pub(crate) fn restore_notice_events(
    session_id: &str,
    notices: &[SessionRestoreNotice],
) -> Vec<StreamEvent> {
    notices
        .iter()
        .map(|notice| {
            let (title, message, reason, recoverable) = restore_notice_render(*notice);
            session_events::recovery_notice_event(
                session_id,
                &format!("notice-{reason}-{session_id}"),
                title,
                message,
                reason,
                recoverable,
            )
        })
        .collect()
}

/// Failure of a session restore attempt. Carries the selector's parity
/// classification and notices so the Fresh outcome (which has no restored
/// session to attach them to) can still be surfaced to the UI by callers
/// that own an `AppHandle`.
#[derive(Debug)]
pub(crate) struct SessionRestoreFailure {
    pub reason: String,
    pub parity: SessionParityStatus,
    pub notices: Vec<SessionRestoreNotice>,
}

impl std::fmt::Display for SessionRestoreFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

pub(crate) struct RestoredSession {
    pub(crate) session: Arc<AgentSession>,
    pub(crate) session_id: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) missing_api_key: bool,
    pub(crate) latest_workflow: Option<WorkflowState>,
    pub(crate) latest_delivery: Option<DeliverySummary>,
    pub(crate) pending_confirms: Vec<PendingConfirmDescriptor>,
    pub(crate) active_tool_calls: Vec<ActiveToolCallDescriptor>,
    /// Restore-selection outcome, surfaced to the UI by
    /// `emit_restored_session_startup` through the existing recovery-notice
    /// stream event.
    pub(crate) restore_notices: Vec<SessionRestoreNotice>,
    /// Which durable source the session was restored from.
    pub(crate) restore_source: SessionRestoreSource,
}

pub(crate) async fn restore_session_from_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<RestoredSession, SessionRestoreFailure> {
    // Load both durable sources, then let the pure selector decide. The
    // journal is a fallback-only source: snapshots are never deleted here
    // and a healthy snapshot always wins.
    let snapshot_outcome = try_load_session_snapshot(session_id);
    let journal_store = SessionJournalStore::new(
        crate::agent::session_mutation::session_journal_root(),
        session_id.to_string(),
    )
    .ok();
    let journal_outcome = match &journal_store {
        Some(store) => match store.load() {
            Ok(result) if result.events.is_empty() => Ok(None),
            Ok(result) => Ok(Some(SessionJournalLoad::from(result))),
            Err(error) => Err(error),
        },
        None => Err(JournalError::UnsafeSessionId(session_id.to_string())),
    };
    let decision = choose_session_restore_source(snapshot_outcome, journal_outcome);
    crate::app_log!(
        "INFO",
        "[session_restore] session {}: source={:?}, parity={:?}, notices={:?}",
        session_id,
        decision.source,
        decision.parity,
        decision.notices
    );
    // Quarantine a corrupt journal BEFORE building the session so the
    // builder's journal initialization starts a fresh generation. The
    // corrupt file is renamed aside, never rewritten or deleted.
    if decision
        .notices
        .contains(&SessionRestoreNotice::JournalQuarantined)
    {
        if let Some(store) = &journal_store {
            match store.quarantine_active() {
                Ok(Some(path)) => crate::app_log!(
                    "WARN",
                    "[session_journal] quarantined corrupt journal for session {}: {}",
                    session_id,
                    path.display()
                ),
                Ok(None) => {}
                Err(error) => crate::app_log!(
                    "WARN",
                    "[session_journal] failed to quarantine journal for session {}: {:?}",
                    session_id,
                    error
                ),
            }
        }
    }
    let Some(snapshot) = decision.snapshot else {
        // Fresh outcome: the failure carries the notices so callers with an
        // AppHandle can still tell the user what happened (e.g. that their
        // journal was quarantined and preserved aside).
        return Err(SessionRestoreFailure {
            reason: format!("no durable state remains for session '{session_id}'"),
            parity: decision.parity,
            notices: decision.notices,
        });
    };
    let restore_source = decision.source;
    let decision_parity = decision.parity;
    let decision_notices = decision.notices.clone();
    let failure = move |reason: String| SessionRestoreFailure {
        reason,
        parity: decision_parity,
        notices: decision_notices.clone(),
    };
    let provider = normalize_provider(Some(&snapshot.provider));
    let profile = state.profiles.get_active_profile();
    let credentials = state
        .credential_resolver()
        .resolve(&provider, profile.as_ref())
        .map_err(|error| failure(error.to_string()))?;
    let latest_workflow = snapshot.latest_workflow.clone();
    let latest_delivery = snapshot.latest_delivery.clone();
    let pending_confirms = snapshot.pending_confirms.clone();
    let active_tool_calls = snapshot.active_tool_calls.clone();

    let model_str = snapshot.model.clone();
    let working_dir = resolve_safe_workspace_path(&snapshot.working_dir).map_err(&failure)?;
    let (session, missing_api_key) = build_agent_session(BuildAgentSessionRequest {
        session_id: snapshot.session_id.clone(),
        provider: provider.clone(),
        model: model_str.clone(),
        api_key: &credentials.api_key,
        api_base: credentials.api_base.as_deref(),
        working_dir: &working_dir,
        pending_confirms: state.pending_confirms.clone(),
        existing_context_window_tokens: snapshot.context_window_tokens,
    })
    .await
    .map_err(&failure)?;
    if restore_source == SessionRestoreSource::JournalProjection {
        // The journal is already authoritative — re-appending a baseline
        // would duplicate its own final state back into it (two extra events
        // on every restore). Snapshot restores keep the baseline so the
        // journal catches up with the imported snapshot.
        session.restore_state_without_baseline(
            snapshot.messages,
            snapshot.summary,
            snapshot.latest_turn,
            snapshot.goal_ledger,
            snapshot.a2a_state,
        );
    } else {
        session.restore_state_with_provenance(
            snapshot.messages,
            snapshot.summary,
            snapshot.latest_turn,
            snapshot.goal_ledger,
            snapshot.a2a_state,
            Some(crate::agent::session_mutation::SessionRestoreProvenance {
                snapshot_schema_version: snapshot.schema_version,
            }),
        );
    }
    // Mark as Resuming before registering so list_sessions can report "resuming"
    // during restore; emit_restored_session_startup will promote to Running.
    *lock_unpoisoned(&session.status) = SessionStatus::Resuming;
    let session = Arc::new(session);
    register_and_dispatch_session_start(state, session.clone(), &snapshot.session_id).await;
    if let Err(error) = save_session_snapshot_with_workflow(state, &session).await {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    Ok(RestoredSession {
        session,
        session_id: snapshot.session_id,
        provider,
        model: model_str,
        missing_api_key,
        latest_workflow,
        latest_delivery,
        pending_confirms,
        active_tool_calls,
        restore_notices: decision.notices,
        restore_source,
    })
}

pub(crate) async fn resume_existing_session(
    app_handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    session_id: &str,
    session: Arc<AgentSession>,
) -> Result<SessionCreated, String> {
    let session = upgrade_missing_key_session_if_possible(app_handle, state, session).await?;
    session.resume(app_handle);
    let _ = session
        .harness
        .dispatch_session_start_event(session_id)
        .await;
    if let Err(error) = save_session_snapshot_with_workflow(state, &session).await {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    emit_session_projection_and_delivery(state, app_handle, session_id, &session).await;
    Ok(SessionCreated {
        session_id: session_id.to_string(),
        provider: normalize_provider(Some(&session.agent_type)),
        model: session.model_id.clone(),
        missing_api_key: session.is_waiting_for_api_key(),
    })
}

pub(crate) async fn emit_restored_session_startup(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    restored: &RestoredSession,
) {
    emit_session_started(
        app_handle,
        session_id,
        &restored.provider,
        &restored.model,
        restored.session.context_window_tokens,
    );
    // Phase 1.4: stream "resuming" after session_started so the frontend has
    // a session row to update while projections replay.
    crate::transcript::emit_stream_event(
        app_handle,
        session_events::session_status_event(session_id, "resuming"),
    );
    // Phase 1.5: replay pending confirmation descriptors as non-interactive
    // interrupted blocks so the user can see what was pending before the
    // app was closed. These use replayed_interrupted=true so the frontend
    // renders them the same way as closeInterruptedConfirmBlocks with reason
    // "session_restored".
    for descriptor in &restored.pending_confirms {
        crate::transcript::emit_stream_event(
            app_handle,
            session_events::pending_confirm_replay_event(session_id, descriptor),
        );
        crate::transcript::emit_stream_event(
            app_handle,
            session_events::pending_confirm_response_replay_event(session_id, descriptor),
        );
    }
    // Phase 1.6: replay active tool-call descriptors as interrupted/completed
    // blocks. Each descriptor produces a ToolCallStart followed by an error
    // ToolCallResult so the user sees which tool was in-flight and that it
    // was terminated by session restore. The restored session's harness
    // active_tool_call_descriptors registry stays empty — these are only
    // visual markers, not re-associated tool processes.
    for descriptor in &restored.active_tool_calls {
        for event in session_events::active_tool_call_replay_events(session_id, descriptor) {
            crate::transcript::emit_stream_event(app_handle, event);
        }
    }
    if let Some(workflow) = &restored.latest_workflow {
        state
            .workflow_states
            .write()
            .await
            .insert(session_id.to_string(), workflow.clone());
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::WorkflowUpdated {
                session_id: session_id.to_string(),
                state: workflow.clone(),
            },
        );
    }
    restored.session.emit_latest_turn_projection(app_handle);
    if let Some(delivery) = &restored.latest_delivery {
        state
            .delivery_states
            .write()
            .await
            .insert(session_id.to_string(), delivery.clone());
        emit_delivery_summary(app_handle, session_id, delivery.clone());
    }
    if restored.missing_api_key {
        emit_missing_api_key_notice(app_handle, session_id, &restored.provider);
    }
    // Task 5: surface restore-selection notices (journal recovery, torn
    // prefix, quarantine) through the existing recovery-notice transport.
    for event in restore_notice_events(session_id, &restored.restore_notices) {
        crate::transcript::emit_stream_event(app_handle, event);
    }
    // Replay complete — promote the session to Running and stream the transition.
    *lock_unpoisoned(&restored.session.status) = SessionStatus::Running;
    crate::transcript::emit_stream_event(
        app_handle,
        session_events::session_status_event(session_id, "running"),
    );
}

pub(crate) async fn upgrade_missing_key_session_if_possible(
    app_handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    session: Arc<AgentSession>,
) -> Result<Arc<AgentSession>, String> {
    if !session.is_waiting_for_api_key() {
        return Ok(session);
    }

    let snapshot = session.snapshot();
    let provider = normalize_provider(Some(&snapshot.provider));
    let profile = state.profiles.get_active_profile();
    let credentials = state
        .credential_resolver()
        .resolve(&provider, profile.as_ref())
        .map_err(|error| error.to_string())?;
    if credentials.api_key.trim().is_empty() && settings::provider_requires_api_key(&provider) {
        return Ok(session);
    }

    let working_dir = resolve_safe_workspace_path(&snapshot.working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));
    let model_str = snapshot.model.clone();
    let external_tools = harness.external_mcp_tool_definitions().await;
    let provider_profiles = settings::load_configured_provider_profiles();
    let adapter = build_adapter_with_profiles(
        &provider,
        &credentials.api_key,
        &model_str,
        credentials.api_base.as_deref(),
        &provider_profiles,
        external_tools,
    )
    .map_err(|error| error.to_string())?;
    let system_prompt = harness.build_system_prompt(&provider, &working_dir).await;
    let upgraded = AgentSession::new(
        snapshot.session_id.clone(),
        provider.clone(),
        adapter,
        harness,
        system_prompt,
        snapshot.context_window_tokens,
    );
    // Attach the shadow journal before restore so the upgraded session keeps
    // journaling and the restore baseline is recorded (journal-first order).
    upgraded.initialize_session_journal();
    upgraded.restore_state(
        snapshot.messages,
        snapshot.summary,
        snapshot.latest_turn,
        snapshot.goal_ledger,
        snapshot.a2a_state,
    );
    let upgraded = Arc::new(upgraded);
    state
        .register_session(snapshot.session_id.clone(), upgraded.clone())
        .await;
    register_gateway_session_best_effort(&snapshot.session_id, &upgraded).await;
    let _ = upgraded
        .harness
        .dispatch_session_start_event(&snapshot.session_id)
        .await;
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::SessionStarted {
            session_id: snapshot.session_id,
            agent_type: provider,
            model: model_str,
            context_window_tokens: upgraded.context_window_tokens,
        },
    );
    Ok(upgraded)
}

pub(crate) async fn save_session_snapshot_with_workflow(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> Result<(), String> {
    let snapshot = session_snapshot_with_workflow_state(state, session).await;
    save_session_snapshot(&snapshot)
}

pub(crate) async fn session_snapshot_with_workflow_state(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> AgentSessionSnapshot {
    let latest_workflow = state.workflow_states.read().await.get(&session.id).cloned();
    let latest_delivery = state.delivery_states.read().await.get(&session.id).cloned();
    let pending_confirms = session
        .harness
        .pending_confirm_descriptors
        .read()
        .await
        .clone();
    let active_tool_calls = session
        .harness
        .active_tool_call_descriptors
        .read()
        .await
        .clone();
    let mut snapshot = session.snapshot();
    if let Some(workflow) = latest_workflow {
        snapshot = snapshot.with_latest_workflow(workflow);
    }
    if let Some(delivery) = latest_delivery {
        snapshot = snapshot.with_latest_delivery(delivery);
    }
    snapshot = snapshot.with_pending_confirms(pending_confirms);
    snapshot = snapshot.with_active_tool_calls(active_tool_calls);
    // Stamp the journal position so the restore selector can tell when this
    // snapshot has fallen behind the journal. `journal_generation` is left
    // unset: sequences are generation-independent and the selector never
    // compares the label, so recording a constant would be noise.
    if let Some(journal) = session.session_journal_handle() {
        snapshot.journal_sequence = journal.store().last_sequence();
    }
    snapshot
}

pub(crate) async fn list_session_infos_for_state(
    state: &Arc<AppState>,
    snapshots: Vec<AgentSessionSnapshot>,
) -> Vec<SessionInfo> {
    let mut by_id = std::collections::HashMap::new();
    for snapshot in snapshots {
        by_id.insert(
            snapshot.session_id.clone(),
            SessionInfo {
                id: snapshot.session_id,
                provider: snapshot.provider,
                model: snapshot.model,
                status: "stopped".to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: snapshot.latest_workflow,
                latest_delivery: snapshot.latest_delivery,
            },
        );
    }

    let sessions = state.sessions.read().await;
    let workflow_states = state.workflow_states.read().await;
    let delivery_states = state.delivery_states.read().await;
    for (id, session) in sessions.iter() {
        let status = session.status.lock();
        let snapshot = session.snapshot();
        by_id.insert(
            id.clone(),
            SessionInfo {
                id: id.clone(),
                provider: session.agent_type.clone(),
                model: session.model_id.clone(),
                status: status.as_str().to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: workflow_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_workflow),
                latest_delivery: delivery_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_delivery),
            },
        );
    }

    let mut result: Vec<_> = by_id.into_values().collect();
    result.sort_by(|a, b| {
        b.updated_at_ms
            .unwrap_or(0)
            .cmp(&a.updated_at_ms.unwrap_or(0))
    });
    result
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SessionInfo>, String> {
    let snapshots = list_session_snapshots()?;
    Ok(list_session_infos_for_state(&state, snapshots).await)
}

#[tauri::command]
pub async fn kill_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if let Some(s) = state.sessions.read().await.get(&session_id).cloned() {
        s.kill(&app_handle);
        let _ = s.harness.dispatch_session_stop_event(&session_id).await;
        unregister_gateway_session_best_effort(&session_id).await;
        if let Err(error) = save_session_snapshot_with_workflow(&state, &s).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
    }
    crate::log_store::log_event(
        "INFO",
        "session",
        &format!("session '{session_id}' killed"),
        Some(&session_id),
    );
    Ok(())
}

#[tauri::command]
pub async fn delete_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if let Some(s) = state.sessions.read().await.get(&session_id).cloned() {
        s.kill(&app_handle);
        let _ = s.harness.dispatch_session_stop_event(&session_id).await;
    }
    state.unregister_session(&session_id).await;
    unregister_gateway_session_best_effort(&session_id).await;
    state.workflow_states.write().await.remove(&session_id);
    state.delivery_states.write().await.remove(&session_id);
    if let Err(error) = delete_session_snapshot(&session_id) {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    if let Err(error) = crate::transcript::delete_transcript(&session_id) {
        crate::app_log!("WARN", "[transcript] {}", error);
    }
    crate::log_store::log_event(
        "INFO",
        "session",
        &format!("session '{session_id}' deleted"),
        Some(&session_id),
    );
    Ok(())
}

/// Pure selection strategy: picks which snapshot to restore at startup.
///
/// 1. If `active_session_id` matches a snapshot that isn't already live, use it.
/// 2. Otherwise, pick the most-recent non-live snapshot.
/// 3. Returns `None` when snapshots is empty or every candidate is already live.
///
/// Snapshots are expected to be sorted by `updated_at_ms` descending (as
/// returned by `list_session_snapshots`).
pub(crate) fn choose_startup_snapshot<'a>(
    active_session_id: Option<&str>,
    snapshots: &'a [AgentSessionSnapshot],
    live_session_ids: &std::collections::HashSet<String>,
) -> Option<&'a AgentSessionSnapshot> {
    if snapshots.is_empty() {
        return None;
    }

    if let Some(active_id) = active_session_id {
        if let Some(snapshot) = snapshots.iter().find(|s| s.session_id == active_id) {
            if !live_session_ids.contains(&snapshot.session_id) {
                return Some(snapshot);
            }
        }
    }

    snapshots
        .iter()
        .find(|s| !live_session_ids.contains(&s.session_id))
}

/// Called once at app startup. Restores the active session (or the most
/// recent session) from its snapshot.  Never blocks startup — failures are
/// logged and swallowed.
///
/// Phase 1.7: if the active snapshot is corrupted/unreadable, or if an active
/// restore fails, surface a user-visible recovery notice and attempt fallback
/// to another snapshot before starting fresh.
pub(crate) async fn startup_restore_active_session(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
) {
    let metadata = match crate::app_metadata::load_app_metadata() {
        Ok(m) => m,
        Err(e) => {
            crate::app_log!("WARN", "[startup_restore] failed to load app metadata: {e}");
            crate::app_metadata::AppMetadata::default()
        }
    };

    let snapshots = match list_session_snapshots() {
        Ok(s) => s,
        Err(e) => {
            crate::app_log!(
                "WARN",
                "[startup_restore] failed to list session snapshots: {e}"
            );
            return;
        }
    };

    let live_ids: std::collections::HashSet<String> =
        state.sessions.read().await.keys().cloned().collect();

    // Phase 1.7: detect if the active session snapshot was skipped by
    // list_session_snapshots (corrupted/unreadable/unsafe). The snapshot
    // file exists on disk but could not be deserialized or validated.
    let active_corrupted = match &metadata.active_session_id {
        Some(active_id) if !live_ids.contains(active_id) => {
            !snapshots.iter().any(|s| s.session_id == *active_id)
        }
        _ => false,
    };

    if active_corrupted {
        if let Some(ref active_id) = metadata.active_session_id {
            crate::app_log!(
                "WARN",
                "[startup_restore] active session {active_id} snapshot is corrupted or unreadable — choosing fallback"
            );
            crate::transcript::emit_stream_event(
                app_handle,
                session_events::recovery_notice_event(
                    active_id,
                    &format!("notice-corrupt-{active_id}"),
                    "Session data was unreadable",
                    "Forge could not restore your last session because its saved data was corrupted or unreadable.",
                    "snapshot_corrupted",
                    true,
                ),
            );
        }
    }

    let chosen =
        choose_startup_snapshot(metadata.active_session_id.as_deref(), &snapshots, &live_ids);

    let Some(snapshot) = chosen else {
        return;
    };

    let session_id = snapshot.session_id.clone();
    let is_fallback = active_corrupted && Some(&session_id) != metadata.active_session_id.as_ref();

    match restore_session_from_snapshot(state, &session_id).await {
        Ok(restored) => {
            emit_restored_session_startup(state, app_handle, &session_id, &restored).await;
            if is_fallback {
                crate::transcript::emit_stream_event(
                    app_handle,
                    session_events::recovery_notice_event(
                        &session_id,
                        &format!("notice-fallback-{session_id}"),
                        "Recovered with a previous session",
                        "Forge started with a different saved session because the last one could not be restored.",
                        "snapshot_fallback_used",
                        true,
                    ),
                );
            }
        }
        Err(failure) => {
            crate::app_log!(
                "WARN",
                "[startup_restore] failed to restore session {session_id}: {}",
                failure.reason
            );
            // Surface the selector's notices (e.g. journal quarantined and
            // preserved aside) — the Fresh outcome has no restored session
            // to carry them, so they are emitted here.
            for event in restore_notice_events(&session_id, &failure.notices) {
                crate::transcript::emit_stream_event(app_handle, event);
            }

            // Phase 1.7: if the failed session was the active one, try a fallback.
            let active_id_matches = metadata
                .active_session_id
                .as_ref()
                .is_some_and(|id| id == &session_id);
            let fallback = if active_id_matches {
                snapshots
                    .iter()
                    .find(|s| s.session_id != session_id && !live_ids.contains(&s.session_id))
            } else {
                None
            };

            if let Some(fallback_snapshot) = fallback {
                let fallback_id = fallback_snapshot.session_id.clone();
                match restore_session_from_snapshot(state, &fallback_id).await {
                    Ok(fallback_restored) => {
                        emit_restored_session_startup(
                            state,
                            app_handle,
                            &fallback_id,
                            &fallback_restored,
                        )
                        .await;
                        crate::transcript::emit_stream_event(
                            app_handle,
                            session_events::recovery_notice_event(
                                &fallback_id,
                                &format!("notice-fallback-{fallback_id}"),
                                "Recovered with a previous session",
                                "Forge started with a different saved session because the last one could not be restored.",
                                "snapshot_fallback_used",
                                true,
                            ),
                        );
                        return;
                    }
                    Err(fallback_err) => {
                        crate::app_log!(
                            "WARN",
                            "[startup_restore] fallback restore of {fallback_id} also failed: {}",
                            fallback_err.reason
                        );
                        for event in restore_notice_events(&fallback_id, &fallback_err.notices) {
                            crate::transcript::emit_stream_event(app_handle, event);
                        }
                    }
                }
            }

            // No fallback (or fallback also failed) — surface notice and start
            // fresh. Skip the generic notice when the selector already
            // emitted a specific one (RestoreFailedFreshStart renders the
            // same reason).
            if failure.notices.is_empty() {
                crate::transcript::emit_stream_event(
                    app_handle,
                    session_events::recovery_notice_event(
                        &session_id,
                        &format!("notice-restore-fail-{session_id}"),
                        "Session restore failed",
                        "Forge could not restore your last session and started fresh. Your data is safe.",
                        "snapshot_restore_failed",
                        false,
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "session_lifecycle_tests.rs"]
mod tests;
