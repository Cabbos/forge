use std::sync::Arc;

use crate::agent::session_events;
use crate::agent::snapshot::PendingConfirmDescriptor;
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

pub(crate) async fn confirm_response_for_state(
    state: &Arc<AppState>,
    block_id: &str,
    approved: bool,
) -> Result<Option<StreamEvent>, String> {
    let response_event = pending_confirm_descriptor_for_state(state, block_id)
        .await
        .map(|(session_id, descriptor)| match descriptor {
            Some(descriptor) => {
                session_events::confirm_response_event(&session_id, &descriptor, approved)
            }
            None => session_events::minimal_confirm_response_event(&session_id, block_id, approved),
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

async fn pending_confirm_descriptor_for_state(
    state: &Arc<AppState>,
    block_id: &str,
) -> Option<(String, Option<PendingConfirmDescriptor>)> {
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
            return Some((session_id.clone(), Some(descriptor)));
        }
    }

    if sessions.len() == 1 {
        return sessions
            .first()
            .map(|(session_id, _)| (session_id.clone(), None));
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
    use crate::harness::Harness;
    use crate::ipc::confirmations::confirm_response_for_state;
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
        harness
            .pending_confirm_descriptors
            .write()
            .await
            .push(PendingConfirmDescriptor::new(
                "confirm-1".to_string(),
                "Allow write?".to_string(),
                "file_write".to_string(),
                42,
            ));
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
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "confirm-1");
                assert_eq!(question.as_deref(), Some("Allow write?"));
                assert_eq!(kind.as_deref(), Some("file_write"));
                assert_eq!(approved, Some(false));
                assert_eq!(reason.as_deref(), Some("user_response"));
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
