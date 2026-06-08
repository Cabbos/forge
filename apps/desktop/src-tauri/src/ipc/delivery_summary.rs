use std::sync::Arc;

use crate::agent::delivery_state::{
    build_delivery_summary, DeliveryCheckpointInput, DeliveryRecordInput, DeliveryRuntimeInput,
};
use crate::agent::session::AgentPreviewStatusUpdate;
use crate::agent::turn_state::AgentTurnState;
use crate::ipc::project_checkpoint::project_checkpoint_status_for_session;
use crate::ipc::project_runtime::project_runtime_status_for_session;
use crate::protocol::events::{DeliverySummary, StreamEvent};
use crate::protocol::BlockId;
use crate::state::AppState;

pub(crate) struct DeliveryPreviewEvidence {
    pub(crate) project_path: Option<String>,
    pub(crate) running: bool,
    pub(crate) can_start: bool,
    pub(crate) can_open: bool,
    pub(crate) label: String,
    pub(crate) url: Option<String>,
}

pub(crate) struct BuiltDeliverySummary {
    pub(crate) summary: DeliverySummary,
    pub(crate) preview_evidence: Option<DeliveryPreviewEvidence>,
    pub(crate) checkpoint_evidence: Option<(bool, bool, bool)>,
}

pub(crate) fn emit_delivery_summary(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    summary: DeliverySummary,
) {
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::DeliverySummary {
            session_id: session_id.to_string(),
            block_id: BlockId::new().to_string(),
            summary,
        },
    );
}

pub(crate) async fn build_delivery_summary_for_session(
    state: &Arc<AppState>,
    session_id: &str,
    latest_turn: Option<&AgentTurnState>,
    record: Option<DeliveryRecordInput>,
) -> BuiltDeliverySummary {
    let mut preview_evidence: Option<DeliveryPreviewEvidence> = None;
    let runtime = match project_runtime_status_for_session(state, Some(session_id)).await {
        Ok(status) => {
            let project_path = status.working_dir.clone();
            preview_evidence = Some(DeliveryPreviewEvidence {
                project_path: Some(project_path.clone()),
                running: status.running,
                can_start: status.can_start,
                can_open: status.can_open,
                label: status.message.clone(),
                url: Some(status.url.clone()),
            });
            Some(DeliveryRuntimeInput {
                project_path: Some(project_path),
                running: status.running,
                can_start: status.can_start,
                can_open: status.can_open,
            })
        }
        Err(error) => {
            crate::app_log!("WARN", "[delivery_state] runtime status failed: {}", error);
            None
        }
    };
    let mut checkpoint_evidence: Option<(bool, bool, bool)> = None;
    let checkpoint = match project_checkpoint_status_for_session(state, Some(session_id)).await {
        Ok(status) => {
            let has_checkpoint = status.last_checkpoint.is_some();
            checkpoint_evidence = Some((
                status.is_git_repo,
                status.dirty,
                has_checkpoint && status.restorable,
            ));
            Some(DeliveryCheckpointInput {
                is_git_repo: status.is_git_repo,
                dirty: status.dirty,
                has_checkpoint,
                restorable: status.restorable,
            })
        }
        Err(error) => {
            crate::app_log!(
                "WARN",
                "[delivery_state] checkpoint status failed: {}",
                error
            );
            None
        }
    };
    let summary = build_delivery_summary(
        runtime,
        checkpoint,
        latest_turn.map(|turn| &turn.verification),
        record,
    );
    BuiltDeliverySummary {
        summary,
        preview_evidence,
        checkpoint_evidence,
    }
}

pub(crate) async fn build_store_emit_delivery_summary(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    latest_turn: Option<&AgentTurnState>,
    record: Option<DeliveryRecordInput>,
) {
    let built = build_delivery_summary_for_session(state, session_id, latest_turn, record).await;
    let summary = built.summary;
    if let Some(session) = state.sessions.read().await.get(session_id).cloned() {
        if let Some(preview) = built.preview_evidence.as_ref() {
            let label = if preview.label.trim().is_empty() {
                summary.preview_label.as_str()
            } else {
                preview.label.as_str()
            };
            session.record_latest_preview_status(
                AgentPreviewStatusUpdate {
                    project_path: preview.project_path.as_deref(),
                    running: preview.running,
                    can_start: preview.can_start,
                    can_open: preview.can_open,
                    label,
                    url: preview.url.as_deref(),
                },
                app_handle,
            );
        }
        if let Some((is_git_repo, dirty, has_checkpoint)) = built.checkpoint_evidence {
            session.record_latest_checkpoint_status(
                is_git_repo,
                dirty,
                has_checkpoint,
                &summary.checkpoint_label,
                app_handle,
            );
        }
        session.record_latest_delivery_summary(&summary, app_handle);
    }
    state
        .delivery_states
        .write()
        .await
        .insert(session_id.to_string(), summary.clone());
    emit_delivery_summary(app_handle, session_id, summary);
}
