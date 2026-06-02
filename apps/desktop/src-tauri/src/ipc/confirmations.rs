use std::sync::Arc;

use crate::state::AppState;

pub(crate) async fn confirm_response_for_state(
    state: &Arc<AppState>,
    block_id: &str,
    approved: bool,
) -> Result<(), String> {
    let sender = { state.pending_confirms.write().await.remove(block_id) };
    match sender {
        Some(tx) => tx
            .send(approved)
            .map_err(|_| format!("Confirm receiver already closed for: {block_id}")),
        None => Err(format!("No pending confirm for: {block_id}")),
    }
}

#[tauri::command]
pub async fn confirm_response(
    state: tauri::State<'_, Arc<AppState>>,
    block_id: String,
    approved: bool,
) -> Result<(), String> {
    confirm_response_for_state(&state, &block_id, approved).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::harness::Harness;
    use crate::ipc::confirmations::confirm_response_for_state;
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

        confirm_response_for_state(&state, "block-1", true)
            .await
            .expect("confirm response");

        assert!(rx.await.expect("receiver"));
        assert!(!state.pending_confirms.read().await.contains_key("block-1"));
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
