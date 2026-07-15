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
