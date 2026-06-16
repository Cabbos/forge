use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::session::AgentSession;
use crate::agent::snapshot::AgentSessionSnapshot;
use crate::harness::Harness;
use crate::ipc::session_lifecycle::{
    list_session_infos_for_state, session_snapshot_with_workflow_state,
};
use crate::protocol::events::DeliverySummary;
use crate::state::AppState;

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
