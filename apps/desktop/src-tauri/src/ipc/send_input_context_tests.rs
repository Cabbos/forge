use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::session::AgentSession;
use crate::harness::capability::CapabilityKind;
use crate::harness::Harness;
use crate::ipc::send_input_context::{
    capability_names_by_kind, reserve_turn_then_record_user_message,
};

#[test]
fn turn_capability_names_omit_internal_infrastructure() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-turn-capabilities-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let harness = Harness::new(workspace.clone());

    let skills = capability_names_by_kind(&harness, CapabilityKind::Skill);
    let hooks = capability_names_by_kind(&harness, CapabilityKind::Hook);

    assert!(!skills.iter().any(|name| name == "Skill Loader"));
    assert!(!hooks.iter().any(|name| name == "Logging Hook"));
    assert!(!hooks.iter().any(|name| name == "File System Audit Hook"));
    assert!(hooks.iter().any(|name| name == "Sensitive Content Guard"));
    assert!(hooks.iter().any(|name| name == "Workspace Boundary Guard"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn busy_session_does_not_record_user_message_before_turn_reservation() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-busy-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        None,
    );
    let _active_turn = session.reserve_turn().expect("first turn should reserve");
    let mut recorded = Vec::new();

    let error = reserve_turn_then_record_user_message(&session, "session-1", "继续", |event| {
        recorded.push(event)
    })
    .expect_err("busy session should reject before recording");

    assert!(error.contains("上一条请求"));
    assert!(recorded.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn stopped_session_does_not_record_user_message_before_turn_reservation() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-stopped-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        None,
    );
    session.running.store(false, Ordering::SeqCst);
    let mut recorded = Vec::new();

    let error = reserve_turn_then_record_user_message(&session, "session-1", "继续", |event| {
        recorded.push(event)
    })
    .expect_err("stopped session should reject before recording");

    assert!(error.contains("Session is not running"));
    assert!(recorded.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}
