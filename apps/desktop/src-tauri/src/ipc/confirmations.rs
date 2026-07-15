use std::collections::HashSet;
use std::sync::Arc;

use crate::agent::session_events;
use crate::agent::snapshot::PendingConfirmDescriptor;
use crate::harness::permission_ledger::PermissionLedgerEvent;
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PendingConfirmationSnapshot {
    pub count: usize,
    pub live_sender_count: usize,
    pub descriptor_count: usize,
    pub decisions: Vec<PendingConfirmationDecisionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PendingConfirmationDecisionSnapshot {
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub workspace_path: String,
    pub question: String,
    pub kind: String,
    pub has_sender: bool,
    pub has_descriptor: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_evidence: Option<PermissionLedgerEvent>,
    pub created_at_ms: u64,
}

pub(crate) async fn confirm_response_for_state(
    state: &Arc<AppState>,
    block_id: &str,
    approved: bool,
) -> Result<Option<StreamEvent>, String> {
    let response_event = pending_confirm_descriptor_for_state(state, block_id)
        .await
        .map(|(session_id, descriptor, workspace)| match descriptor {
            Some(descriptor) => {
                session_events::confirm_response_event(&session_id, &descriptor, approved)
            }
            None => session_events::minimal_confirm_response_event(
                &session_id,
                block_id,
                approved,
                Some(workspace.as_path()),
            ),
        });
    let sender = { state.pending_confirms.write().await.remove(block_id) };
    match sender {
        Some(tx) => {
            tx.send(approved)
                .map_err(|_| format!("Confirm receiver already closed for: {block_id}"))?;
            Ok(response_event)
        }
        None => Err(format!("No pending confirm for: {block_id}")),
    }
}

pub(crate) async fn pending_confirmation_snapshot_for_state(
    state: &Arc<AppState>,
) -> PendingConfirmationSnapshot {
    let sender_ids: HashSet<String> = state
        .pending_confirms
        .read()
        .await
        .keys()
        .cloned()
        .collect();
    let sessions_guard = state.sessions.read().await;
    let sessions: Vec<_> = sessions_guard
        .iter()
        .map(|(session_id, session)| (session_id.clone(), Arc::clone(session)))
        .collect();
    drop(sessions_guard);

    let mut decisions = Vec::new();
    let mut descriptor_ids = HashSet::new();
    for (session_id, session) in &sessions {
        let descriptors = session
            .harness
            .pending_confirm_descriptors
            .read()
            .await
            .clone();
        push_pending_confirm_descriptors(
            &mut decisions,
            &mut descriptor_ids,
            &sender_ids,
            Some(session_id.as_str()),
            &session.harness.working_dir,
            descriptors,
        );
    }

    let root_descriptors = state
        .harness
        .pending_confirm_descriptors
        .read()
        .await
        .clone();
    push_pending_confirm_descriptors(
        &mut decisions,
        &mut descriptor_ids,
        &sender_ids,
        None,
        &state.harness.working_dir,
        root_descriptors,
    );
    let descriptor_count = decisions.len();

    for block_id in &sender_ids {
        if descriptor_ids.contains(block_id) {
            continue;
        }
        decisions.push(PendingConfirmationDecisionSnapshot {
            block_id: block_id.clone(),
            session_id: None,
            workspace_path: state.harness.working_dir.to_string_lossy().into_owned(),
            question: String::new(),
            kind: "unknown".to_string(),
            has_sender: true,
            has_descriptor: false,
            permission_evidence: None,
            created_at_ms: 0,
        });
    }
    decisions.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.block_id.cmp(&right.block_id))
    });

    PendingConfirmationSnapshot {
        count: decisions.len(),
        live_sender_count: sender_ids.len(),
        descriptor_count,
        decisions,
    }
}

fn push_pending_confirm_descriptors(
    decisions: &mut Vec<PendingConfirmationDecisionSnapshot>,
    descriptor_ids: &mut HashSet<String>,
    sender_ids: &HashSet<String>,
    session_id: Option<&str>,
    workspace_path: &std::path::Path,
    descriptors: Vec<PendingConfirmDescriptor>,
) {
    for descriptor in descriptors {
        if !descriptor_ids.insert(descriptor.block_id.clone()) {
            continue;
        }
        decisions.push(PendingConfirmationDecisionSnapshot {
            block_id: descriptor.block_id.clone(),
            session_id: session_id.map(ToOwned::to_owned),
            workspace_path: workspace_path.to_string_lossy().into_owned(),
            question: descriptor.question,
            kind: descriptor.kind,
            has_sender: sender_ids.contains(&descriptor.block_id),
            has_descriptor: true,
            permission_evidence: descriptor.permission_evidence,
            created_at_ms: descriptor.created_at_ms,
        });
    }
}

async fn pending_confirm_descriptor_for_state(
    state: &Arc<AppState>,
    block_id: &str,
) -> Option<(String, Option<PendingConfirmDescriptor>, std::path::PathBuf)> {
    let sessions_guard = state.sessions.read().await;
    let sessions: Vec<_> = sessions_guard
        .iter()
        .map(|(session_id, session)| (session_id.clone(), Arc::clone(session)))
        .collect();
    drop(sessions_guard);

    for (session_id, session) in &sessions {
        let descriptors = session.harness.pending_confirm_descriptors.read().await;
        if let Some(descriptor) = descriptors
            .iter()
            .find(|descriptor| descriptor.block_id == block_id)
            .cloned()
        {
            return Some((
                session_id.clone(),
                Some(descriptor),
                session.harness.working_dir.clone(),
            ));
        }
    }

    if sessions.len() == 1 {
        return sessions.first().map(|(session_id, session)| {
            (
                session_id.clone(),
                None,
                session.harness.working_dir.clone(),
            )
        });
    }

    None
}

#[tauri::command]
pub async fn confirm_response(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    block_id: String,
    approved: bool,
) -> Result<(), String> {
    let event = confirm_response_for_state(&state, &block_id, approved).await?;
    if let Some(event) = event {
        crate::transcript::emit_stream_event(&app_handle, event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::session::AgentSession;
    use crate::agent::snapshot::PendingConfirmDescriptor;
    use crate::harness::permission_ledger::{
        PermissionLedgerEvent, PermissionLedgerEventKind, PermissionRiskTier,
    };
    use crate::harness::permissions::PermissionMode;
    use crate::harness::Harness;
    use crate::ipc::confirmations::{
        confirm_response_for_state, pending_confirmation_snapshot_for_state,
    };
    use crate::protocol::events::StreamEvent;
    use crate::state::AppState;

    #[tokio::test]
    async fn confirm_response_for_state_resolves_and_removes_pending_sender() {
        let state = Arc::new(AppState::new(Arc::new(Harness::new(std::env::temp_dir()))));
        let (tx, rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("block-1".to_string(), tx);

        let event = confirm_response_for_state(&state, "block-1", true)
            .await
            .expect("confirm response");

        assert!(event.is_none());
        assert!(rx.await.expect("receiver"));
        assert!(!state.pending_confirms.read().await.contains_key("block-1"));
    }

    #[tokio::test]
    async fn pending_confirmation_snapshot_reports_descriptor_sender_consistency() {
        let nonce = uuid::Uuid::now_v7();
        let workspace =
            std::env::temp_dir().join(format!("forge-pending-confirm-snapshot-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let state = Arc::new(AppState::new(harness.clone()));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            harness.clone(),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;
        harness.pending_confirm_descriptors.write().await.push(
            PendingConfirmDescriptor::new(
                "confirm-snapshot".to_string(),
                "Allow snapshot?".to_string(),
                "file_write".to_string(),
                7,
            )
            .with_permission_evidence(PermissionLedgerEvent {
                kind: PermissionLedgerEventKind::ManualRequired,
                workspace_path: workspace.to_string_lossy().into_owned(),
                session_id: Some("session-1".to_string()),
                risk_tier: PermissionRiskTier::Caution,
                affected_files: vec!["src/lib.rs".to_string()],
                operation: "write_to_file".to_string(),
                permission_mode: PermissionMode::ManualConfirm,
                reason: "manual_confirm_requires_user_response".to_string(),
            }),
        );
        let (tx, _rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("confirm-snapshot".to_string(), tx);

        let snapshot = pending_confirmation_snapshot_for_state(&state).await;

        assert_eq!(snapshot.count, 1);
        assert_eq!(snapshot.live_sender_count, 1);
        assert_eq!(snapshot.descriptor_count, 1);
        let decision = snapshot.decisions.first().expect("pending decision");
        assert_eq!(decision.session_id.as_deref(), Some("session-1"));
        assert_eq!(decision.block_id, "confirm-snapshot");
        assert_eq!(decision.kind, "file_write");
        assert!(decision.has_sender);
        assert!(decision.has_descriptor);
        assert_eq!(
            decision
                .permission_evidence
                .as_ref()
                .expect("permission evidence")
                .operation,
            "write_to_file"
        );

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn pending_confirmation_snapshot_reports_restored_descriptor_without_sender() {
        let nonce = uuid::Uuid::now_v7();
        let workspace =
            std::env::temp_dir().join(format!("forge-pending-confirm-restored-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let state = Arc::new(AppState::new(harness.clone()));
        let session = Arc::new(AgentSession::new(
            "session-restored".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            harness.clone(),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-restored".to_string(), session)
            .await;
        harness
            .pending_confirm_descriptors
            .write()
            .await
            .push(PendingConfirmDescriptor::new(
                "confirm-restored".to_string(),
                "Allow restored?".to_string(),
                "permission".to_string(),
                9,
            ));

        let snapshot = pending_confirmation_snapshot_for_state(&state).await;

        assert_eq!(snapshot.count, 1);
        assert_eq!(snapshot.live_sender_count, 0);
        assert_eq!(snapshot.descriptor_count, 1);
        let decision = snapshot.decisions.first().expect("pending decision");
        assert_eq!(decision.session_id.as_deref(), Some("session-restored"));
        assert_eq!(decision.block_id, "confirm-restored");
        assert!(!decision.has_sender);
        assert!(decision.has_descriptor);

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn pending_confirmation_snapshot_reports_sender_without_descriptor() {
        let state = Arc::new(AppState::new(Arc::new(Harness::new(std::env::temp_dir()))));
        let (tx, _rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("sender-only".to_string(), tx);

        let snapshot = pending_confirmation_snapshot_for_state(&state).await;

        assert_eq!(snapshot.count, 1);
        assert_eq!(snapshot.live_sender_count, 1);
        assert_eq!(snapshot.descriptor_count, 0);
        let decision = snapshot.decisions.first().expect("pending decision");
        assert_eq!(decision.block_id, "sender-only");
        assert!(decision.has_sender);
        assert!(!decision.has_descriptor);
    }

    #[tokio::test]
    async fn confirm_response_for_state_returns_replayable_response_event() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-confirm-response-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let state = Arc::new(AppState::new(harness.clone()));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            harness.clone(),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;
        harness.pending_confirm_descriptors.write().await.push(
            PendingConfirmDescriptor::new(
                "confirm-1".to_string(),
                "Allow write?".to_string(),
                "file_write".to_string(),
                42,
            )
            .with_permission_evidence(PermissionLedgerEvent {
                kind: PermissionLedgerEventKind::ManualRequired,
                workspace_path: workspace.to_string_lossy().into_owned(),
                session_id: Some("session-1".to_string()),
                risk_tier: PermissionRiskTier::Caution,
                affected_files: vec!["src/main.rs".to_string()],
                operation: "write_to_file".to_string(),
                permission_mode: PermissionMode::ManualConfirm,
                reason: "manual_confirm_requires_user_response".to_string(),
            }),
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("confirm-1".to_string(), tx);

        let event = confirm_response_for_state(&state, "confirm-1", false)
            .await
            .expect("confirm response")
            .expect("response event");

        assert!(!rx.await.expect("receiver"));
        match event {
            StreamEvent::ConfirmResponse {
                session_id,
                block_id,
                question,
                kind,
                approved,
                reason,
                permission_evidence,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "confirm-1");
                assert_eq!(question.as_deref(), Some("Allow write?"));
                assert_eq!(kind.as_deref(), Some("file_write"));
                assert_eq!(approved, Some(false));
                assert_eq!(reason.as_deref(), Some("user_response"));
                let evidence = permission_evidence.expect("permission evidence");
                assert_eq!(evidence.kind, PermissionLedgerEventKind::UserDeclined);
                assert_eq!(evidence.session_id.as_deref(), Some("session-1"));
                assert_eq!(evidence.affected_files, vec!["src/main.rs".to_string()]);
                assert_eq!(evidence.permission_mode, PermissionMode::ManualConfirm);
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn confirm_response_for_state_returns_user_approved_evidence() {
        let nonce = uuid::Uuid::now_v7();
        let workspace =
            std::env::temp_dir().join(format!("forge-confirm-approved-evidence-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let state = Arc::new(AppState::new(harness.clone()));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            harness.clone(),
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;
        harness.pending_confirm_descriptors.write().await.push(
            PendingConfirmDescriptor::new(
                "confirm-approve".to_string(),
                "Allow write?".to_string(),
                "file_write".to_string(),
                42,
            )
            .with_permission_evidence(PermissionLedgerEvent {
                kind: PermissionLedgerEventKind::ManualRequired,
                workspace_path: workspace.to_string_lossy().into_owned(),
                session_id: Some("session-1".to_string()),
                risk_tier: PermissionRiskTier::Caution,
                affected_files: vec!["src/main.rs".to_string()],
                operation: "write_to_file".to_string(),
                permission_mode: PermissionMode::ManualConfirm,
                reason: "manual_confirm_requires_user_response".to_string(),
            }),
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("confirm-approve".to_string(), tx);

        let event = confirm_response_for_state(&state, "confirm-approve", true)
            .await
            .expect("confirm response")
            .expect("response event");

        assert!(rx.await.expect("receiver"));
        match event {
            StreamEvent::ConfirmResponse {
                permission_evidence,
                approved,
                ..
            } => {
                assert_eq!(approved, Some(true));
                let evidence = permission_evidence.expect("permission evidence");
                assert_eq!(evidence.kind, PermissionLedgerEventKind::UserApproved);
                assert_eq!(evidence.reason, "user_response");
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn confirm_response_for_state_returns_minimal_event_when_descriptor_is_missing() {
        let nonce = uuid::Uuid::now_v7();
        let workspace =
            std::env::temp_dir().join(format!("forge-confirm-response-minimal-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let state = Arc::new(AppState::new(harness.clone()));
        let session = Arc::new(AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
            harness,
            "system".to_string(),
            Some(128_000),
        ));
        state
            .register_session("session-1".to_string(), session)
            .await;
        let (tx, rx) = tokio::sync::oneshot::channel();
        state
            .pending_confirms
            .write()
            .await
            .insert("confirm-1".to_string(), tx);

        let event = confirm_response_for_state(&state, "confirm-1", true)
            .await
            .expect("confirm response")
            .expect("minimal response event");

        assert!(rx.await.expect("receiver"));
        match event {
            StreamEvent::ConfirmResponse {
                session_id,
                block_id,
                question,
                kind,
                approved,
                reason,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "confirm-1");
                assert_eq!(question, None);
                assert_eq!(kind, None);
                assert_eq!(approved, Some(true));
                assert_eq!(reason.as_deref(), Some("user_response"));
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn confirm_response_for_state_rejects_missing_block() {
        let state = Arc::new(AppState::new(Arc::new(Harness::new(std::env::temp_dir()))));

        let error = confirm_response_for_state(&state, "missing-block", false)
            .await
            .expect_err("missing block should fail");

        assert!(error.contains("No pending confirm"));
    }
}
