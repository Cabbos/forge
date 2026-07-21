use std::collections::HashSet;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::session::AgentSession;
use crate::agent::session::SessionStatus;
use crate::agent::snapshot::{
    delete_session_snapshot, save_session_snapshot, AgentSessionSnapshot,
};
use crate::agent::snapshot::{
    ActiveToolCallDescriptor, ActiveToolCallStatus, PendingConfirmDescriptor,
};
use crate::credential_store::{CredentialStore, MemoryCredentialStore};
use crate::harness::Harness;
use crate::ipc::session_lifecycle::{
    choose_startup_snapshot, gateway_session_ids_for_shutdown, gateway_session_info_for_session,
    gateway_session_infos_for_state, list_session_infos_for_state, restore_session_from_snapshot,
    session_snapshot_with_workflow_state,
};
use crate::protocol::events::DeliverySummary;
use crate::settings::Settings;
use crate::state::AppState;

fn memory_credential_store_for_saved_references() -> Arc<dyn CredentialStore> {
    let store = Arc::new(MemoryCredentialStore::default());
    for reference in Settings::load().credential_refs.values() {
        store
            .put(reference, "test-credential")
            .expect("seed test credential reference");
    }
    store
}

fn test_agent_session(id: &str, workspace: &std::path::Path) -> Arc<AgentSession> {
    Arc::new(AgentSession::new(
        id.to_string(),
        "deepseek".to_string(),
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
        Arc::new(Harness::new(workspace.to_path_buf())),
        "system".to_string(),
        Some(128_000),
    ))
}

#[tokio::test]
async fn session_snapshot_with_workflow_state_uses_session_workspace_and_latest_delivery() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-snapshot-session-{nonce}"));
    let default_workspace = std::env::temp_dir().join(format!("forge-snapshot-default-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    state.delivery_states.write().await.insert(
        "session-1".to_string(),
        DeliverySummary {
            project_path: Some(session_workspace.to_string_lossy().to_string()),
            preview_label: "预览未运行".to_string(),
            checkpoint_label: "还没有检查点".to_string(),
            next_action: "下一步：启动预览。".to_string(),
            verification_label: None,
            verification_status: None,
            verification_command: None,
            record_label: None,
            record_status: None,
            record_target_pages: Vec::new(),
        },
    );

    let snapshot = session_snapshot_with_workflow_state(&state, &session).await;

    assert_eq!(
        std::path::PathBuf::from(snapshot.working_dir)
            .canonicalize()
            .expect("snapshot workspace"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_eq!(
        snapshot
            .latest_delivery
            .and_then(|delivery| delivery.project_path),
        Some(session_workspace.to_string_lossy().to_string())
    );

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
}

#[tokio::test]
async fn list_session_infos_prefers_live_session_state_over_stale_snapshot() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-list-session-{nonce}"));
    let stale_workspace = std::env::temp_dir().join(format!("forge-list-stale-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&stale_workspace).expect("stale workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        stale_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;
    state.delivery_states.write().await.insert(
        "session-1".to_string(),
        DeliverySummary {
            project_path: Some(session_workspace.to_string_lossy().to_string()),
            preview_label: "预览运行中".to_string(),
            checkpoint_label: "检查点已就绪".to_string(),
            next_action: "下一步：交付状态可以继续验收。".to_string(),
            verification_label: Some("检查已通过".to_string()),
            verification_status: Some("passed".to_string()),
            verification_command: Some("npm run build".to_string()),
            record_label: None,
            record_status: None,
            record_target_pages: Vec::new(),
        },
    );
    let snapshot = AgentSessionSnapshot::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        "stale-model".to_string(),
        stale_workspace.to_string_lossy().to_string(),
        Vec::new(),
        None,
        Some(128_000),
    )
    .with_latest_delivery(DeliverySummary {
        project_path: Some(stale_workspace.to_string_lossy().to_string()),
        preview_label: "预览未运行".to_string(),
        checkpoint_label: "当前不是 Git 项目".to_string(),
        next_action: "下一步：启动预览。".to_string(),
        verification_label: None,
        verification_status: None,
        verification_command: None,
        record_label: None,
        record_status: None,
        record_target_pages: Vec::new(),
    });

    let infos = list_session_infos_for_state(&state, vec![snapshot]).await;

    assert_eq!(infos.len(), 1);
    let info = &infos[0];
    assert_eq!(info.id, "session-1");
    assert_eq!(info.status, "running");
    assert_eq!(info.model, "deepseek-chat");
    assert_eq!(
        std::path::PathBuf::from(info.working_dir.as_deref().expect("working dir"))
            .canonicalize()
            .expect("info workspace"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_eq!(
        info.latest_delivery
            .as_ref()
            .and_then(|delivery| delivery.project_path.clone()),
        Some(session_workspace.to_string_lossy().to_string())
    );

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(stale_workspace);
}

#[tokio::test]
async fn gateway_session_info_for_session_uses_live_session_metadata() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-gateway-info-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let session = test_agent_session("gateway-session", &workspace);

    let info = gateway_session_info_for_session("gateway-session", &session);

    assert_eq!(info.session_id, "gateway-session");
    assert_eq!(info.provider, "deepseek");
    assert_eq!(info.model, "deepseek-chat");
    assert_eq!(info.workspace_path, workspace.to_string_lossy());
    assert!(info.created_at_ms > 0);
    assert_eq!(info.owner_pid, Some(std::process::id()));
    assert!(info.last_seen_at_ms.is_some_and(|seen_at| seen_at > 0));
    assert!(
        !info.restored_from_registry,
        "desktop re-registration should mark the gateway entry as live"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn gateway_session_ids_for_shutdown_returns_sorted_live_session_ids() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-gateway-shutdown-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    state
        .register_session(
            "session-b".to_string(),
            test_agent_session("session-b", &workspace),
        )
        .await;
    state
        .register_session(
            "session-a".to_string(),
            test_agent_session("session-a", &workspace),
        )
        .await;

    let ids = gateway_session_ids_for_shutdown(&state).await;

    assert_eq!(ids, vec!["session-a".to_string(), "session-b".to_string()]);

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn gateway_session_infos_for_state_returns_sorted_live_payloads() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-gateway-heartbeat-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    state
        .register_session(
            "session-b".to_string(),
            test_agent_session("session-b", &workspace),
        )
        .await;
    state
        .register_session(
            "session-a".to_string(),
            test_agent_session("session-a", &workspace),
        )
        .await;

    let infos = gateway_session_infos_for_state(&state).await;

    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].session_id, "session-a");
    assert_eq!(infos[1].session_id, "session-b");
    assert!(infos
        .iter()
        .all(|info| info.owner_pid == Some(std::process::id())));
    assert!(infos.iter().all(|info| info.last_seen_at_ms.is_some()));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn session_snapshot_with_workflow_state_includes_live_descriptors() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-snapshot-descriptors-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        session_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "descriptor-session".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let confirm = PendingConfirmDescriptor::new(
        "confirm-1".to_string(),
        "Allow write?".to_string(),
        "file_write".to_string(),
        42,
    );
    session
        .harness
        .pending_confirm_descriptors
        .write()
        .await
        .push(confirm);

    let tool = ActiveToolCallDescriptor::new(
        "tool-1".to_string(),
        "write_to_file".to_string(),
        serde_json::json!({"path": "file.txt"}),
        100,
    )
    .with_status(ActiveToolCallStatus::AwaitingResult);
    session
        .harness
        .active_tool_call_descriptors
        .write()
        .await
        .push(tool);

    let snapshot = session_snapshot_with_workflow_state(&state, &session).await;

    assert_eq!(snapshot.pending_confirms.len(), 1);
    assert_eq!(snapshot.pending_confirms[0].block_id, "confirm-1");
    assert_eq!(snapshot.active_tool_calls.len(), 1);
    assert_eq!(snapshot.active_tool_calls[0].block_id, "tool-1");
    assert_eq!(
        snapshot.active_tool_calls[0].status,
        ActiveToolCallStatus::AwaitingResult
    );

    let _ = std::fs::remove_dir_all(session_workspace);
}

#[tokio::test]
async fn restore_session_from_snapshot_carries_pending_confirms_without_fake_sender() {
    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-pending-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-pending-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new_with_credential_store(
        Arc::new(Harness::new(workspace.clone())),
        memory_credential_store_for_saved_references(),
    ));
    let descriptor = PendingConfirmDescriptor::new(
        "confirm-restore-1".to_string(),
        "Allow write?".to_string(),
        "file_write".to_string(),
        42,
    );
    let snapshot = AgentSessionSnapshot::new(
        session_id.clone(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        workspace.to_string_lossy().to_string(),
        Vec::new(),
        None,
        Some(128_000),
    )
    .with_pending_confirms(vec![descriptor]);
    save_session_snapshot(&snapshot).expect("save snapshot");

    let restored = restore_session_from_snapshot(&state, &session_id)
        .await
        .expect("restore session");

    assert_eq!(restored.pending_confirms.len(), 1);
    assert_eq!(restored.pending_confirms[0].block_id, "confirm-restore-1");
    assert!(
        !state
            .pending_confirms
            .read()
            .await
            .contains_key("confirm-restore-1"),
        "restored confirm descriptors must not register fake confirm senders"
    );

    let _ = delete_session_snapshot(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn restore_session_from_snapshot_carries_active_tool_calls_without_fake_registry() {
    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-tool-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-tool-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new_with_credential_store(
        Arc::new(Harness::new(workspace.clone())),
        memory_credential_store_for_saved_references(),
    ));
    let descriptor = ActiveToolCallDescriptor::new(
        "tool-restore-1".to_string(),
        "write_to_file".to_string(),
        serde_json::json!({"path": "file.txt", "content": "hello"}),
        200,
    )
    .with_status(ActiveToolCallStatus::AwaitingResult);
    let snapshot = AgentSessionSnapshot::new(
        session_id.clone(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        workspace.to_string_lossy().to_string(),
        Vec::new(),
        None,
        Some(128_000),
    )
    .with_active_tool_calls(vec![descriptor]);
    save_session_snapshot(&snapshot).expect("save snapshot");

    let restored = restore_session_from_snapshot(&state, &session_id)
        .await
        .expect("restore session");

    // RestoredSession carries the descriptor for replay
    assert_eq!(restored.active_tool_calls.len(), 1);
    assert_eq!(restored.active_tool_calls[0].block_id, "tool-restore-1");
    assert_eq!(restored.active_tool_calls[0].tool_name, "write_to_file");
    assert_eq!(
        restored.active_tool_calls[0].tool_input,
        serde_json::json!({"path": "file.txt", "content": "hello"})
    );

    // Harness active_tool_call_descriptors registry must stay empty —
    // the restored session must NOT register fake tool handles.
    let harness_descriptors = restored
        .session
        .harness
        .active_tool_call_descriptors
        .read()
        .await
        .clone();
    assert!(
        harness_descriptors.is_empty(),
        "restored harness active_tool_call_descriptors must be empty; descriptors are replay-only"
    );

    let _ = delete_session_snapshot(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}

// ── choose_startup_snapshot selection strategy ──

fn snapshot_with(id: &str, updated_at_ms: u64) -> AgentSessionSnapshot {
    AgentSessionSnapshot::new(
        id.to_string(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        "/tmp/workspace".to_string(),
        Vec::new(),
        None,
        Some(128_000),
    )
    .with_updated_at_ms(updated_at_ms)
}

/// Extension to set `updated_at_ms` directly for test determinism.
impl AgentSessionSnapshot {
    fn with_updated_at_ms(mut self, ms: u64) -> Self {
        self.updated_at_ms = ms;
        self
    }
}

#[test]
fn choose_startup_snapshot_active_wins_over_more_recent() {
    let active = snapshot_with("active-session", 100);
    let newer = snapshot_with("newer-session", 200);
    let snapshots = vec![newer.clone(), active.clone()]; // newer first (most recent)

    let result = choose_startup_snapshot(Some("active-session"), &snapshots, &HashSet::new());

    assert_eq!(result.unwrap().session_id, "active-session");
}

#[test]
fn choose_startup_snapshot_missing_active_falls_back_to_most_recent() {
    let older = snapshot_with("older-session", 100);
    let newer = snapshot_with("newer-session", 200);
    let snapshots = vec![newer.clone(), older.clone()];

    let result = choose_startup_snapshot(Some("nonexistent-session"), &snapshots, &HashSet::new());

    assert_eq!(result.unwrap().session_id, "newer-session");
}

#[test]
fn choose_startup_snapshot_no_active_id_falls_back_to_most_recent() {
    let older = snapshot_with("older-session", 100);
    let newer = snapshot_with("newer-session", 200);
    let snapshots = vec![newer.clone(), older.clone()];

    let result = choose_startup_snapshot(None, &snapshots, &HashSet::new());

    assert_eq!(result.unwrap().session_id, "newer-session");
}

#[test]
fn choose_startup_snapshot_empty_snapshots_returns_none() {
    let result = choose_startup_snapshot(Some("any-id"), &[], &HashSet::new());

    assert!(result.is_none());
}

#[test]
fn choose_startup_snapshot_skips_already_live_active_session() {
    let active = snapshot_with("active-session", 200);
    let other = snapshot_with("other-session", 100);
    let snapshots = vec![active.clone(), other.clone()];
    let mut live = HashSet::new();
    live.insert("active-session".to_string());

    let result = choose_startup_snapshot(Some("active-session"), &snapshots, &live);

    // Active is live, so fall back to the most recent non-live session
    assert_eq!(result.unwrap().session_id, "other-session");
}

#[test]
fn choose_startup_snapshot_returns_none_when_all_live() {
    let s1 = snapshot_with("session-1", 200);
    let s2 = snapshot_with("session-2", 100);
    let snapshots = vec![s1, s2];
    let mut live = HashSet::new();
    live.insert("session-1".to_string());
    live.insert("session-2".to_string());

    let result = choose_startup_snapshot(None, &snapshots, &live);

    assert!(result.is_none());
}

#[test]
fn choose_startup_snapshot_active_nonexistent_with_no_live_falls_back() {
    // Active ID points to a session that was deleted; fall back to most recent
    let only = snapshot_with("only-session", 300);
    let snapshots = vec![only.clone()];

    let result = choose_startup_snapshot(Some("deleted-session"), &snapshots, &HashSet::new());

    assert_eq!(result.unwrap().session_id, "only-session");
}

// ── metadata default → fallback contract ──

#[test]
fn app_metadata_default_has_no_active_session_id() {
    let meta = crate::app_metadata::AppMetadata::default();
    assert!(
        meta.active_session_id.is_none(),
        "AppMetadata::default() must have active_session_id=None so that \
         startup_restore falls back to most-recent snapshot on metadata load failure"
    );
}

#[test]
fn metadata_load_failure_falls_back_to_most_recent_snapshot() {
    // Simulates what happens when load_app_metadata() returns Err:
    // we use AppMetadata::default() (active_session_id = None), which
    // lets choose_startup_snapshot fall back to the most recent non-live snapshot.
    let meta = crate::app_metadata::AppMetadata::default();
    assert!(meta.active_session_id.is_none());

    let older = snapshot_with("older-session", 100);
    let newer = snapshot_with("newer-session", 200);
    let snapshots = vec![newer.clone(), older.clone()];

    let result = choose_startup_snapshot(
        meta.active_session_id.as_deref(),
        &snapshots,
        &HashSet::new(),
    );

    assert_eq!(result.unwrap().session_id, "newer-session");
}

#[tokio::test]
async fn list_session_infos_reports_live_resuming_status() {
    use crate::agent::session_guards::lock_unpoisoned;

    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-list-resuming-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));
    let session = Arc::new(AgentSession::new(
        "session-resuming".to_string(),
        "deepseek".to_string(),
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    // Simulate the state that restore_session_from_snapshot sets before
    // register_and_dispatch_session_start.
    *lock_unpoisoned(&session.status) = SessionStatus::Resuming;
    state
        .register_session("session-resuming".to_string(), session)
        .await;

    let infos = list_session_infos_for_state(&state, vec![]).await;
    assert_eq!(infos.len(), 1);
    let info = &infos[0];
    assert_eq!(info.id, "session-resuming");
    assert_eq!(info.status, "resuming");

    let _ = std::fs::remove_dir_all(workspace);
}

// ── Phase 1.7: corruption fallback selection ──

/// When the active session snapshot is corrupted (skipped by list, not in
/// snapshots vec), `choose_startup_snapshot` correctly falls back to the
/// most recent non-live snapshot instead of returning None.
#[test]
fn choose_startup_snapshot_corrupted_active_falls_back_to_valid() {
    // "session-valid" is the only valid snapshot; "session-corrupt" was
    // skipped by list_session_snapshots due to parse/safety failure.
    let valid = snapshot_with("session-valid", 200);
    let snapshots = vec![valid.clone()];

    // active_session_id points to the corrupted snapshot that is not in the list.
    let result = choose_startup_snapshot(Some("session-corrupt"), &snapshots, &HashSet::new());

    assert_eq!(result.unwrap().session_id, "session-valid");
}

/// When the active snapshot is corrupted AND all other snapshots are
/// already live, choose_startup_snapshot returns None — no valid candidate.
#[test]
fn choose_startup_snapshot_corrupted_active_with_all_others_live_returns_none() {
    let other = snapshot_with("other-session", 100);
    let snapshots = vec![other];
    let mut live = HashSet::new();
    live.insert("other-session".to_string());

    let result = choose_startup_snapshot(Some("session-corrupt"), &snapshots, &live);

    assert!(result.is_none());
}

/// When both the active ID is missing (corrupted) and there are no
/// snapshots at all, returns None — consistent with empty case.
#[test]
fn choose_startup_snapshot_corrupted_active_empty_list_returns_none() {
    let result = choose_startup_snapshot(Some("session-corrupt"), &[], &HashSet::new());
    assert!(result.is_none());
}

// ── Phase 1.8: Provider alias normalization during restore ──────────

/// Snapshots preserve the raw provider string (e.g. "claude").
/// `restore_session_from_snapshot` and `list_session_infos_for_state`
/// normalise aliases ("claude" → "anthropic", "gpt" → "openai") so the
/// restored session runs with the canonical provider ID.
#[test]
fn provider_snapshot_raw_vs_restore_normalized_contract() {
    use crate::agent::provider_capabilities::normalize_provider;

    // The snapshot layer must never normalise — it preserves raw provider.
    assert_eq!(normalize_provider(Some("claude")), "anthropic");
    assert_eq!(normalize_provider(Some("gpt")), "openai");
    assert_eq!(normalize_provider(Some("deepseek")), "deepseek");
    assert_eq!(
        normalize_provider(Some("custom-provider")),
        "custom-provider"
    );

    // A snapshot storing "claude" should round-trip as "claude".
    let snapshot = AgentSessionSnapshot::new(
        "alias-session".to_string(),
        "claude".to_string(),
        "claude-sonnet-4-6".to_string(),
        "/workspace".to_string(),
        Vec::new(),
        None,
        Some(200_000),
    );
    assert_eq!(snapshot.provider, "claude");

    let json = serde_json::to_string(&snapshot).expect("serialize");
    let restored: AgentSessionSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.provider, "claude");
}

#[tokio::test]
async fn restore_session_from_snapshot_normalizes_provider_alias() {
    use crate::agent::provider_capabilities::normalize_provider;

    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-alias-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-alias-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    // Snapshot saved with alias "claude" — the raw provider string.
    let snapshot = AgentSessionSnapshot::new(
        session_id.clone(),
        "claude".to_string(),
        "claude-sonnet-4-6".to_string(),
        workspace.to_string_lossy().to_string(),
        Vec::new(),
        None,
        Some(200_000),
    );
    save_session_snapshot(&snapshot).expect("save snapshot");

    // restore_session_from_snapshot normalises via normalize_provider.
    let restored = restore_session_from_snapshot(&state, &session_id)
        .await
        .expect("restore session");

    // Provider should have been normalised.
    let normalised = normalize_provider(Some(&snapshot.provider));
    assert_eq!(normalised, "anthropic");
    assert_eq!(restored.provider, normalised);
    assert_eq!(restored.session.agent_type, normalised);

    let _ = delete_session_snapshot(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}

// ── Task 5: journal-backed restore selection matrix ──

use crate::adapters::base::ChatMessage;
use crate::agent::session_journal::{
    JournalDamage, JournalError, SessionMutation, SessionMutationEnvelope,
    SESSION_JOURNAL_SCHEMA_VERSION,
};
use crate::agent::session_projection::SessionProjection;
use crate::agent::snapshot::SnapshotLoadFailure;
use crate::ipc::session_lifecycle::{
    choose_session_restore_source, SessionJournalLoad, SessionParityStatus, SessionRestoreNotice,
    SessionRestoreSource,
};

fn journal_init_event(session_id: &str, sequence: u64) -> SessionMutationEnvelope {
    SessionMutationEnvelope {
        schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
        event_id: format!("init-{sequence}"),
        session_id: session_id.to_string(),
        sequence,
        created_at_ms: sequence,
        mutation: SessionMutation::SessionInitialized {
            provider: "deepseek".to_string(),
            model: "deepseek-chat".to_string(),
            working_dir: "/tmp/workspace".to_string(),
        },
    }
}

fn journal_message_event(session_id: &str, sequence: u64, text: &str) -> SessionMutationEnvelope {
    SessionMutationEnvelope {
        schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
        event_id: format!("msg-{sequence}"),
        session_id: session_id.to_string(),
        sequence,
        created_at_ms: sequence,
        mutation: SessionMutation::MessageAppended {
            message: ChatMessage::user(text),
        },
    }
}

/// A journal with one init event (sequence 1) plus one message per text,
/// so `texts.len() + 1` is the last sequence.
fn journal_events(session_id: &str, texts: &[&str]) -> Vec<SessionMutationEnvelope> {
    let mut events = vec![journal_init_event(session_id, 1)];
    for (index, text) in texts.iter().enumerate() {
        events.push(journal_message_event(session_id, (index + 2) as u64, text));
    }
    events
}

fn journal_load(events: Vec<SessionMutationEnvelope>, torn_final_line: bool) -> SessionJournalLoad {
    SessionJournalLoad {
        damage: torn_final_line.then_some(JournalDamage::TornFinalLine { line: 99 }),
        events,
    }
}

fn projection_snapshot(events: &[SessionMutationEnvelope]) -> AgentSessionSnapshot {
    SessionProjection::from_events(events)
        .expect("projection")
        .to_snapshot()
}

fn corrupt_snapshot() -> Result<Option<AgentSessionSnapshot>, SnapshotLoadFailure> {
    Err(SnapshotLoadFailure::Corrupt {
        reason: "invalid JSON".to_string(),
    })
}

fn corrupt_interior_journal() -> Result<Option<SessionJournalLoad>, JournalError> {
    Err(JournalError::CorruptInteriorLine { line: 3 })
}

fn message_texts(snapshot: &AgentSessionSnapshot) -> Vec<String> {
    snapshot
        .messages
        .iter()
        .filter_map(|message| message.content.as_str().map(String::from))
        .collect()
}

// Row 1: valid snapshot at the same sequence as a valid journal restores the
// snapshot with healthy parity. Fields the journal never captures
// (latest_workflow, latest_delivery, pending_confirms, active_tool_calls,
// context_window_tokens) must not break parity.
#[test]
fn restore_selector_same_sequence_snapshot_wins_with_healthy_parity() {
    let events = journal_events("session-1", &["hello", "world"]);
    let mut snapshot = projection_snapshot(&events);
    snapshot.journal_sequence = 3;
    snapshot.context_window_tokens = Some(128_000);
    snapshot.pending_confirms = vec![PendingConfirmDescriptor::new(
        "confirm-1".to_string(),
        "Allow write?".to_string(),
        "file_write".to_string(),
        42,
    )];
    snapshot.active_tool_calls = vec![ActiveToolCallDescriptor::new(
        "tool-1".to_string(),
        "read_file".to_string(),
        serde_json::json!({"path": "x"}),
        100,
    )];

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::Healthy);
    assert!(decision.notices.is_empty());
    let payload = decision.snapshot.expect("snapshot payload");
    assert_eq!(message_texts(&payload), vec!["hello", "world"]);
}

// Row 2: valid snapshot behind the journal restores the journal projection.
#[test]
fn restore_selector_snapshot_behind_uses_journal_projection() {
    let events = journal_events("session-1", &["hello", "world"]);
    let mut snapshot = projection_snapshot(&events[..2]); // only "hello"
    snapshot.journal_sequence = 1; // recorded before the last two appends

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::JournalProjection);
    assert_eq!(decision.parity, SessionParityStatus::SnapshotBehind);
    assert!(decision.notices.is_empty());
    let payload = decision.snapshot.expect("journal projection payload");
    assert_eq!(message_texts(&payload), vec!["hello", "world"]);
    assert_eq!(payload.journal_sequence, 3);
}

// Row 3: missing snapshot with a valid journal restores the journal projection.
#[test]
fn restore_selector_missing_snapshot_uses_journal_projection() {
    let events = journal_events("session-1", &["hello"]);

    let decision = choose_session_restore_source(Ok(None), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::JournalProjection);
    assert_eq!(decision.parity, SessionParityStatus::JournalOnly);
    assert!(decision.notices.is_empty());
    let payload = decision.snapshot.expect("journal projection payload");
    assert_eq!(message_texts(&payload), vec!["hello"]);
}

// Row 4: corrupt snapshot with a valid journal restores the journal projection
// and surfaces a recovery notice.
#[test]
fn restore_selector_corrupt_snapshot_uses_journal_with_recovery_notice() {
    let events = journal_events("session-1", &["hello"]);

    let decision =
        choose_session_restore_source(corrupt_snapshot(), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::JournalProjection);
    assert_eq!(decision.parity, SessionParityStatus::JournalOnly);
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::SnapshotRecoveredFromJournal));
    let payload = decision.snapshot.expect("journal projection payload");
    assert_eq!(message_texts(&payload), vec!["hello"]);
}

// Row 5: valid snapshot with a corrupt-interior journal restores the snapshot
// and quarantines the journal.
#[test]
fn restore_selector_corrupt_interior_journal_keeps_snapshot_and_quarantines() {
    let snapshot = snapshot_with("session-1", 100);

    let decision = choose_session_restore_source(Ok(Some(snapshot)), corrupt_interior_journal());

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::CorruptInterior);
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::JournalQuarantined));
    assert!(decision.snapshot.is_some());
}

// Row 6: corrupt snapshot AND corrupt-interior journal leaves nothing durable —
// start fresh with a recovery notice.
#[test]
fn restore_selector_corrupt_snapshot_and_journal_starts_fresh_with_notice() {
    let decision = choose_session_restore_source(corrupt_snapshot(), corrupt_interior_journal());

    assert_eq!(decision.source, SessionRestoreSource::Fresh);
    assert_eq!(decision.parity, SessionParityStatus::CorruptInterior);
    assert!(decision.snapshot.is_none());
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::JournalQuarantined));
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::RestoreFailedFreshStart));
}

// Row 7a: valid snapshot behind a journal with a torn final line restores the
// journal's valid prefix, with a warning.
#[test]
fn restore_selector_torn_final_journal_newer_than_snapshot_uses_prefix_with_warning() {
    let events = journal_events("session-1", &["hello", "world"]);
    let mut snapshot = projection_snapshot(&events[..2]);
    snapshot.journal_sequence = 1;

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, true))));

    assert_eq!(decision.source, SessionRestoreSource::JournalProjection);
    assert_eq!(decision.parity, SessionParityStatus::TornFinalLine);
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::JournalTornFinalLine));
    let payload = decision.snapshot.expect("journal prefix payload");
    assert_eq!(message_texts(&payload), vec!["hello", "world"]);
}

// Row 7b: valid snapshot NOT behind a torn journal keeps the snapshot, with a
// warning.
#[test]
fn restore_selector_torn_final_journal_not_newer_keeps_snapshot_with_warning() {
    let events = journal_events("session-1", &["hello"]);
    let mut snapshot = projection_snapshot(&events);
    snapshot.journal_sequence = 2; // same as the journal's last sequence

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, true))));

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::TornFinalLine);
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::JournalTornFinalLine));
    assert!(decision.snapshot.is_some());
}

// Legacy snapshots (no journal sequence metadata) are never treated as behind:
// the snapshot wins and parity is decided by content comparison.
#[test]
fn restore_selector_legacy_snapshot_with_matching_journal_is_healthy() {
    let events = journal_events("session-1", &["hello"]);
    let snapshot = projection_snapshot(&events); // journal_sequence defaults to 0

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::Healthy);
    assert!(decision.notices.is_empty());
}

// A legacy snapshot whose content diverges from the journal (e.g. the known
// repair_message_history parity gap) still wins, reported as diverged — never
// treated as corrupt.
#[test]
fn restore_selector_legacy_snapshot_diverging_from_journal_reports_diverged() {
    let events = journal_events("session-1", &["hello", "repaired-away"]);
    let mut snapshot = projection_snapshot(&events[..2]); // only "hello"
    snapshot.journal_sequence = 0;

    let decision =
        choose_session_restore_source(Ok(Some(snapshot)), Ok(Some(journal_load(events, false))));

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::Diverged);
    assert!(decision.notices.is_empty());
}

#[test]
fn restore_selector_valid_snapshot_without_journal_is_snapshot_only() {
    let snapshot = snapshot_with("session-1", 100);

    let decision = choose_session_restore_source(Ok(Some(snapshot)), Ok(None));

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::SnapshotOnly);
    assert!(decision.notices.is_empty());
}

#[test]
fn restore_selector_nothing_durable_starts_fresh_without_notices() {
    let decision = choose_session_restore_source(Ok(None), Ok(None));

    assert_eq!(decision.source, SessionRestoreSource::Fresh);
    assert!(decision.snapshot.is_none());
    assert!(decision.notices.is_empty());
}

#[test]
fn restore_selector_corrupt_snapshot_without_journal_starts_fresh_with_notice() {
    let decision = choose_session_restore_source(corrupt_snapshot(), Ok(None));

    assert_eq!(decision.source, SessionRestoreSource::Fresh);
    assert!(decision.snapshot.is_none());
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::RestoreFailedFreshStart));
}

// A journal whose events cannot form a projection (no init event) is
// quarantined like a corrupt-interior journal.
#[test]
fn restore_selector_journal_without_init_event_is_quarantined() {
    let orphan_events = vec![journal_message_event("session-1", 1, "orphan")];
    let snapshot = snapshot_with("session-1", 100);

    let decision = choose_session_restore_source(
        Ok(Some(snapshot)),
        Ok(Some(journal_load(orphan_events, false))),
    );

    assert_eq!(decision.source, SessionRestoreSource::Snapshot);
    assert_eq!(decision.parity, SessionParityStatus::CorruptInterior);
    assert!(decision
        .notices
        .contains(&SessionRestoreNotice::JournalQuarantined));
}

// ── Task 5 review: end-to-end restore with on-disk journals ──

use crate::agent::session_journal::SessionJournalStore;
use crate::agent::snapshot::load_session_snapshot;
use crate::ipc::session_lifecycle::restore_notice_events;
use crate::protocol::events::StreamEvent;

/// Default (`~/.forge`) sessions directory, matching `snapshot.rs` and
/// `session_mutation.rs` root resolution. Tests use UUID session ids and
/// clean up after themselves so they never collide with real data.
fn default_forge_root() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".forge")
}

fn journal_envelope(session_id: &str, mutation: SessionMutation) -> SessionMutationEnvelope {
    SessionMutationEnvelope {
        schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
        event_id: String::new(),
        session_id: session_id.to_string(),
        sequence: 0,
        created_at_ms: 1,
        mutation,
    }
}

fn write_default_journal(
    session_id: &str,
    working_dir: &str,
    texts: &[&str],
) -> SessionJournalStore {
    let store = SessionJournalStore::new(default_forge_root(), session_id.to_string())
        .expect("journal store");
    store
        .append(journal_envelope(
            session_id,
            SessionMutation::SessionInitialized {
                provider: "deepseek".to_string(),
                model: "deepseek-chat".to_string(),
                working_dir: working_dir.to_string(),
            },
        ))
        .expect("append init");
    for text in texts {
        store
            .append(journal_envelope(
                session_id,
                SessionMutation::MessageAppended {
                    message: ChatMessage::user(text),
                },
            ))
            .expect("append message");
    }
    store
}

fn append_raw_journal(path: &std::path::Path, bytes: &[u8]) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open journal");
    file.write_all(bytes).expect("append raw");
    file.sync_all().expect("sync");
}

fn cleanup_default_journal(session_id: &str) {
    let _ = std::fs::remove_dir_all(default_forge_root().join("sessions").join(session_id));
}

// (a) End-to-end: a real corrupt-on-disk journal is quarantined (renamed
// aside) and the session is recovered from its snapshot.
#[tokio::test]
async fn restore_quarantines_corrupt_journal_and_recovers_from_snapshot() {
    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-quarantine-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-quarantine-ws-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new_with_credential_store(
        Arc::new(Harness::new(workspace.clone())),
        memory_credential_store_for_saved_references(),
    ));

    let snapshot = AgentSessionSnapshot::new(
        session_id.clone(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        workspace.to_string_lossy().to_string(),
        vec![ChatMessage::user("snapshot message")],
        None,
        Some(128_000),
    );
    save_session_snapshot(&snapshot).expect("save snapshot");

    // Journal with one valid init event followed by a corrupt interior line.
    let store = write_default_journal(&session_id, &workspace.to_string_lossy(), &[]);
    let journal_path = store.path();
    append_raw_journal(&journal_path, b"this is not json\n");
    drop(store);

    let restored = restore_session_from_snapshot(&state, &session_id)
        .await
        .expect("restore from snapshot");

    assert_eq!(restored.restore_source, SessionRestoreSource::Snapshot);
    assert!(restored
        .restore_notices
        .contains(&SessionRestoreNotice::JournalQuarantined));
    let messages = restored.session.snapshot().messages;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content.as_str(), Some("snapshot message"));

    // The corrupt journal was renamed aside, never deleted or rewritten.
    let journal_dir = default_forge_root().join("sessions").join(&session_id);
    let quarantined = journal_dir.join("mutations.gen0.jsonl");
    assert!(
        quarantined.exists(),
        "corrupt journal preserved aside as a generation"
    );
    let quarantined_contents = std::fs::read_to_string(&quarantined).expect("read quarantined");
    assert!(quarantined_contents.contains("this is not json"));
    // The builder's journal init started a fresh active journal.
    assert!(
        journal_path.exists(),
        "fresh active journal after quarantine"
    );
    let fresh_events = SessionJournalStore::new(default_forge_root(), session_id.clone())
        .expect("loader")
        .load()
        .expect("load fresh journal")
        .events;
    assert!(
        fresh_events
            .iter()
            .all(|event| !matches!(&event.mutation, SessionMutation::MessageAppended { message } if message.content.as_str() == Some("this is not json"))),
        "fresh journal must not replay corrupt data"
    );

    let _ = delete_session_snapshot(&session_id);
    cleanup_default_journal(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}

// (b) End-to-end: journal-only recovery flows through the same restore path,
// and restoring FROM a journal does not append a redundant baseline back into
// it (regression: journal must not grow on restore).
#[tokio::test]
async fn journal_only_restore_recovers_without_growing_journal() {
    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-journal-only-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-journal-only-ws-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new_with_credential_store(
        Arc::new(Harness::new(workspace.clone())),
        memory_credential_store_for_saved_references(),
    ));

    let store = write_default_journal(
        &session_id,
        &workspace.to_string_lossy(),
        &["first", "second"],
    );
    let events_before = store.load().expect("load").events.len();
    assert_eq!(events_before, 3);
    drop(store);

    let restored = restore_session_from_snapshot(&state, &session_id)
        .await
        .expect("journal restore");

    assert_eq!(
        restored.restore_source,
        SessionRestoreSource::JournalProjection
    );
    assert!(restored.restore_notices.is_empty());
    let messages = restored.session.snapshot().messages;
    let texts: Vec<&str> = messages
        .iter()
        .filter_map(|message| message.content.as_str())
        .collect();
    assert_eq!(texts, vec!["first", "second"]);

    // Regression (review item 2): no baseline was appended to the journal the
    // restore just replayed.
    let events_after = SessionJournalStore::new(default_forge_root(), session_id.clone())
        .expect("loader")
        .load()
        .expect("load after restore")
        .events
        .len();
    assert_eq!(
        events_after, events_before,
        "journal-backed restore must not grow the journal"
    );

    // The follow-up snapshot save was stamped with the journal sequence so
    // the "snapshot behind" rule can fire on later restores.
    let saved = load_session_snapshot(&session_id).expect("saved snapshot");
    assert_eq!(saved.journal_sequence, 3);
    assert!(saved.journal_generation.is_none());

    let _ = delete_session_snapshot(&session_id);
    cleanup_default_journal(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}

// (c) Fresh outcome: when both durable sources are unusable, the failure
// carries the selector's notices and they convert into recovery-notice
// stream events (the UI emission path).
#[tokio::test]
async fn fresh_restore_outcome_carries_notices_to_emission_path() {
    let nonce = uuid::Uuid::now_v7();
    let session_id = format!("restore-fresh-{nonce}");
    let workspace = std::env::temp_dir().join(format!("forge-restore-fresh-ws-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    // Corrupt snapshot on disk.
    let snapshot_path = default_forge_root()
        .join("sessions")
        .join(format!("{session_id}.json"));
    std::fs::create_dir_all(snapshot_path.parent().expect("parent")).expect("sessions dir");
    std::fs::write(&snapshot_path, "{ not json").expect("write corrupt snapshot");

    // Corrupt-interior journal on disk.
    let store = write_default_journal(&session_id, &workspace.to_string_lossy(), &[]);
    let journal_path = store.path();
    append_raw_journal(&journal_path, b"this is not json\n");
    drop(store);

    let result = restore_session_from_snapshot(&state, &session_id).await;
    let failure = match result {
        Ok(_) => panic!("both sources unusable must fail"),
        Err(failure) => failure,
    };

    assert_eq!(failure.parity, SessionParityStatus::CorruptInterior);
    assert!(failure
        .notices
        .contains(&SessionRestoreNotice::JournalQuarantined));
    assert!(failure
        .notices
        .contains(&SessionRestoreNotice::RestoreFailedFreshStart));

    // The notices reach the emission path as recovery-notice stream events.
    let events = restore_notice_events(&session_id, &failure.notices);
    assert_eq!(events.len(), 2);
    let reasons: Vec<&str> = events
        .iter()
        .map(|event| match event {
            StreamEvent::RecoveryNotice { reason, .. } => reason.as_str(),
            other => panic!("expected recovery notice, got {other:?}"),
        })
        .collect();
    assert!(reasons.contains(&"journal_quarantined"));
    assert!(reasons.contains(&"snapshot_restore_failed"));

    // The journal was still quarantined (preserved aside) even though nothing
    // could be restored.
    assert!(default_forge_root()
        .join("sessions")
        .join(&session_id)
        .join("mutations.gen0.jsonl")
        .exists());

    let _ = std::fs::remove_file(&snapshot_path);
    cleanup_default_journal(&session_id);
    let _ = std::fs::remove_dir_all(workspace);
}
