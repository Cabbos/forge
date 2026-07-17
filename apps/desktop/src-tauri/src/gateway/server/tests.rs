//! Gateway server dispatch and handler tests (split from server.rs).

use super::loop_tasks::*;
use super::sessions::*;
use super::status::*;
use super::*;

use crate::adapters::base::ChatMessage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_gateway_state() -> GatewayState {
    GatewayState {
        started_at: Instant::now(),
        active_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        sessions: Mutex::new(HashMap::new()),
        session_registry_path: None,
        trigger_store: Arc::new(crate::gateway::webhook::TriggerStore::new()),
        trigger_run_store: Arc::new(crate::gateway::runner::TriggerRunStore::new()),
        session_input_store: Arc::new(crate::gateway::session_input::SessionInputStore::new()),
        loop_event_journal: Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
            tempfile::tempdir()
                .expect("loop runtime tempdir")
                .path()
                .join("loop-events.jsonl"),
        )),
        loop_task_projection_store: Arc::new(
            crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
                tempfile::tempdir()
                    .expect("loop runtime projection tempdir")
                    .path()
                    .join("loop-tasks.json"),
            ),
        ),
        runtime_tasks: Mutex::new(default_runtime_task_map()),
        include_snapshot_sessions: false,
    }
}

// ── dispatch ──────────────────────────────────────────────────────────

#[test]
fn dispatch_ping_returns_ok() {
    let state = GatewayState::new();
    let req = GatewayRequest {
        id: "1".into(),
        method: "ping".into(),
        params: None,
    };
    let reply = dispatch(&state, req);
    match reply {
        GatewayReply::Ok(resp) => {
            assert_eq!(resp.id, "1");
            let ping: PingResult = serde_json::from_value(resp.result).expect("parse ping result");
            assert!(ping.ok);
            assert!(!ping.gateway_version.is_empty());
        }
        _ => panic!("expected Ok reply, got Err"),
    }
}

#[test]
fn dispatch_health_returns_state() {
    let state = GatewayState::new();
    std::thread::sleep(Duration::from_millis(10));
    let req = GatewayRequest {
        id: "2".into(),
        method: "health".into(),
        params: None,
    };
    let reply = dispatch(&state, req);
    match reply {
        GatewayReply::Ok(resp) => {
            assert_eq!(resp.id, "2");
            let health: HealthResult =
                serde_json::from_value(resp.result).expect("parse health result");
            assert!(health.ok);
            // uptime_seconds is u64, so non-negativity is guaranteed
            assert_eq!(health.active_sessions, 0);
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn dispatch_unknown_method_returns_error() {
    let state = GatewayState::new();
    let req = GatewayRequest {
        id: "3".into(),
        method: "nonexistent".into(),
        params: None,
    };
    let reply = dispatch(&state, req);
    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.id, "3");
            assert_eq!(err.error.code, -32601);
            assert!(err.error.message.contains("unknown method"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn create_loop_task_dispatch_persists_task() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection.clone());
    let request = GatewayRequest {
        id: "req-1".to_string(),
        method: "create_loop_task".to_string(),
        params: Some(serde_json::json!({
            "goal": "Ship Level 3 runtime",
            "workspace_path": "/Users/cabbos/project/forge"
        })),
    };

    let GatewayReply::Ok(response) = dispatch(&state, request) else {
        panic!("expected ok response");
    };
    let result: crate::gateway::protocol::LoopTaskResponse =
        serde_json::from_value(response.result).unwrap();
    assert!(result.ok);
    assert_eq!(result.task.goal, "Ship Level 3 runtime");
    assert_eq!(journal.load_all().unwrap().len(), 1);
    assert_eq!(projection.load_or_rebuild(&journal).unwrap().tasks.len(), 1);
}

#[test]
fn create_loop_task_rejects_empty_goal() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "create-empty".into(),
            method: "create_loop_task".into(),
            params: Some(serde_json::json!({ "goal": "   " })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("goal"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn create_loop_task_duplicate_idempotency_returns_original_task() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);
    let request = || GatewayRequest {
        id: "req-create".into(),
        method: "create_loop_task".into(),
        params: Some(serde_json::json!({
            "goal": "Ship Level 3 runtime",
            "idempotency_key": "create:level-3"
        })),
    };

    let first = dispatch(&state, request());
    let second = dispatch(&state, request());

    let first_task = match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::LoopTaskResponse =
                serde_json::from_value(resp.result).expect("parse first create");
            result.task
        }
        _ => panic!("expected first Ok reply"),
    };
    let second_task = match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::LoopTaskResponse =
                serde_json::from_value(resp.result).expect("parse second create");
            result.task
        }
        _ => panic!("expected second Ok reply"),
    };

    assert_eq!(first_task.id, second_task.id);
    assert_eq!(journal.load_all().unwrap().len(), 1);
}

#[test]
fn create_loop_task_duplicate_idempotency_with_different_payload_errors() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let first = dispatch(
        &state,
        GatewayRequest {
            id: "req-create-1".into(),
            method: "create_loop_task".into(),
            params: Some(serde_json::json!({
                "goal": "Ship Level 3 runtime",
                "idempotency_key": "create:level-3",
                "workspace_path": "/repo/one"
            })),
        },
    );
    let second = dispatch(
        &state,
        GatewayRequest {
            id: "req-create-2".into(),
            method: "create_loop_task".into(),
            params: Some(serde_json::json!({
                "goal": "Ship something else",
                "idempotency_key": "create:level-3",
                "workspace_path": "/repo/two"
            })),
        },
    );

    assert!(matches!(first, GatewayReply::Ok(_)));
    match second {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("idempotency conflict"));
        }
        _ => panic!("expected idempotency conflict"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 1);
}

#[test]
fn concurrent_create_loop_task_duplicate_idempotency_appends_once() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let state = Arc::new(GatewayState::new_with_loop_runtime_stores(
        journal.clone(),
        projection,
    ));
    let barrier = Arc::new(std::sync::Barrier::new(12));
    let mut handles = Vec::new();

    for index in 0..12 {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            dispatch(
                &state,
                GatewayRequest {
                    id: format!("req-create-{index}"),
                    method: "create_loop_task".into(),
                    params: Some(serde_json::json!({
                        "goal": "Ship Level 3 runtime",
                        "idempotency_key": "create:level-3",
                        "workspace_path": "/repo"
                    })),
                },
            )
        }));
    }

    let tasks = handles
        .into_iter()
        .map(|handle| match handle.join().unwrap() {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::LoopTaskResponse =
                    serde_json::from_value(resp.result).expect("parse create");
                result.task
            }
            GatewayReply::Err(err) => panic!("unexpected create error: {:?}", err.error),
        })
        .collect::<Vec<_>>();
    let first_id = tasks[0].id.clone();

    assert!(tasks.iter().all(|task| task.id == first_id));
    assert_eq!(journal.load_all().unwrap().len(), 1);
}

#[test]
fn create_loop_task_errors_when_journal_is_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let journal_path = dir.path().join("loop-events.jsonl");
    let valid = serde_json::to_string(
        &crate::loop_runtime::LoopEventEnvelope::task_created_for_test("loop-1", "blocked"),
    )
    .unwrap();
    std::fs::write(&journal_path, format!("{valid}\n{{not json\n")).unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        journal_path,
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let state = GatewayState::new_with_loop_runtime_stores(journal, projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "create-corrupt".into(),
            method: "create_loop_task".into(),
            params: Some(serde_json::json!({ "goal": "blocked" })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("line 2"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn list_loop_tasks_filters_by_status() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-pending",
                "pending",
            ),
        )
        .unwrap();
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-canceled",
                "canceled",
            ),
        )
        .unwrap();
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::task_canceled(
            "loop-canceled".into(),
            Some("done".into()),
            Some("test".into()),
            Some("cancel:loop-canceled:done".into()),
        ))
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal, projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "list-loop".into(),
            method: "list_loop_tasks".into(),
            params: Some(serde_json::json!({
                "statuses": ["pending"],
                "limit": 10
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::ListLoopTasksResult =
                serde_json::from_value(resp.result).expect("parse loop tasks");
            assert_eq!(result.total, 1);
            assert_eq!(result.tasks[0].id, "loop-pending");
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn get_loop_task_rejects_missing_task() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "get-missing-loop".into(),
            method: "get_loop_task".into(),
            params: Some(serde_json::json!({ "task_id": "missing-loop" })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err
                .error
                .message
                .contains("loop task not found: missing-loop"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn request_headless_resume_rejects_missing_task() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "headless-missing".into(),
            method: "request_headless_resume".into(),
            params: Some(serde_json::json!({ "task_id": "missing-loop" })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err
                .error
                .message
                .contains("loop task not found: missing-loop"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn request_headless_resume_default_returns_disabled_without_appending() {
    let state = test_gateway_state();
    state
        .loop_event_journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-disabled",
            "stay bounded",
        ))
        .unwrap();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "headless-default".into(),
            method: "request_headless_resume".into(),
            params: Some(serde_json::json!({ "task_id": "loop-disabled" })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::HeadlessResumeControlResult =
                serde_json::from_value(resp.result).expect("parse headless result");
            assert!(!result.ok);
            assert_eq!(
                result.mode,
                crate::loop_runtime::HeadlessResumeMode::Disabled
            );
            assert!(!result.gateway_can_resume);
            assert!(!result.approval_recorded);
            assert!(result.approval.is_none());
            assert!(result.message.contains("disabled by default"));
            assert!(result
                .message
                .contains("no headless AgentSession was created"));
            assert!(result.task.headless_resume_approval.is_none());
        }
        _ => panic!("expected Ok status reply"),
    }
    assert_eq!(state.loop_event_journal.load_all().unwrap().len(), 1);
}

#[test]
fn request_headless_resume_records_durable_approval_and_replays_idempotently() {
    let state = test_gateway_state();
    state
        .loop_event_journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-approved",
            "record approval",
        ))
        .unwrap();

    let request = |id: &str, idempotency_key: &str| GatewayRequest {
        id: id.to_string(),
        method: "request_headless_resume".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-approved",
            "mode": "approved_for_task",
            "approved_by": "human-reviewer",
            "approved_at_ms": 42,
            "scope": "task",
            "expires_at_ms": 60_042,
            "idempotency_key": idempotency_key
        })),
    };

    let first = dispatch(
        &state,
        request("headless-approve-1", "headless:loop-approved"),
    );
    let second = dispatch(
        &state,
        request("headless-approve-2", "headless:loop-approved"),
    );
    let duplicate_key_changed = dispatch(
        &state,
        request("headless-approve-3", "headless:loop-approved:duplicate-key"),
    );

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::HeadlessResumeControlResult =
                serde_json::from_value(resp.result).expect("parse first headless result");
            assert!(result.ok);
            assert_eq!(
                result.mode,
                crate::loop_runtime::HeadlessResumeMode::ApprovedForTask
            );
            assert!(result.approval_recorded);
            assert!(!result.gateway_can_resume);
            assert!(result
                .message
                .contains("no headless AgentSession was created"));
            assert_eq!(
                result.approval.as_ref().unwrap().approved_by,
                "human-reviewer"
            );
            assert_eq!(
                result.task.headless_resume_mode,
                crate::loop_runtime::HeadlessResumeMode::ApprovedForTask
            );
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::HeadlessResumeControlResult =
                serde_json::from_value(resp.result).expect("parse second headless result");
            assert!(result.ok);
            assert!(!result.approval_recorded);
            assert!(result.task.headless_resume_approval.is_some());
        }
        _ => panic!("expected second Ok reply"),
    }
    match duplicate_key_changed {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::HeadlessResumeControlResult =
                serde_json::from_value(resp.result).expect("parse duplicate-key headless result");
            assert!(result.ok);
            assert!(!result.approval_recorded);
            assert_eq!(
                result.mode,
                crate::loop_runtime::HeadlessResumeMode::ApprovedForTask
            );
        }
        _ => panic!("expected duplicate-key Ok reply"),
    }
    assert_eq!(state.loop_event_journal.load_all().unwrap().len(), 2);

    let replayed = state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
        .unwrap()
        .find("loop-approved")
        .cloned()
        .unwrap();
    assert_eq!(
        replayed.headless_resume_mode,
        crate::loop_runtime::HeadlessResumeMode::ApprovedForTask
    );
    assert_eq!(
        replayed.headless_resume_approval.unwrap().approved_at_ms,
        42
    );
}

#[test]
fn request_headless_resume_conflicting_approval_does_not_append_second_event() {
    let state = test_gateway_state();
    state
        .loop_event_journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-conflicting-approval",
            "record one approval",
        ))
        .unwrap();

    let request = |id: &str, approved_by: &str, idempotency_key: &str| GatewayRequest {
        id: id.to_string(),
        method: "request_headless_resume".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-conflicting-approval",
            "mode": "approved_for_task",
            "approved_by": approved_by,
            "approved_at_ms": 42,
            "scope": "task",
            "expires_at_ms": 60_042,
            "idempotency_key": idempotency_key
        })),
    };

    let first = dispatch(
        &state,
        request(
            "headless-conflicting-approval-1",
            "human-reviewer",
            "headless:conflicting-approval:one",
        ),
    );
    let second = dispatch(
        &state,
        request(
            "headless-conflicting-approval-2",
            "different-reviewer",
            "headless:conflicting-approval:two",
        ),
    );

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::HeadlessResumeControlResult =
                serde_json::from_value(resp.result).expect("parse first headless result");
            assert!(result.ok);
            assert!(result.approval_recorded);
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains(
                "duplicate headless resume approval recorded: loop-conflicting-approval"
            ));
        }
        _ => panic!("expected conflicting approval Err reply"),
    }

    let loaded = state.loop_event_journal.load_all().unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(
        loaded
            .iter()
            .filter(|event| matches!(
                event.event,
                crate::loop_runtime::LoopRuntimeEvent::HeadlessResumeApprovalRecorded { .. }
            ))
            .count(),
        1
    );
    let replayed = state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
        .unwrap()
        .find("loop-conflicting-approval")
        .cloned()
        .unwrap();
    assert_eq!(
        replayed.headless_resume_approval.unwrap().approved_by,
        "human-reviewer"
    );
}

#[test]
fn evaluate_loop_task_completion_returns_typed_result() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let task = crate::loop_runtime::LoopTaskRecord::new_for_test("loop-complete", "ship")
        .with_completion_contract(crate::loop_runtime::LoopCompletionContract {
            required_checks: vec!["build:desktop".to_string()],
            max_gitnexus_risk: None,
            require_docs: false,
            require_commit: false,
            require_review_decision: false,
            stop_on_budget_exceeded: true,
        });
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::task_created(
            task, None, None,
        ))
        .unwrap();
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::evidence_recorded(
            "loop-complete".to_string(),
            crate::loop_runtime::EvidenceRecord::command_for_test("build:desktop", true),
            None,
            None,
        ))
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal, projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "evaluate-loop".into(),
            method: "evaluate_loop_task_completion".into(),
            params: Some(serde_json::json!({ "task_id": "loop-complete" })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::EvaluateLoopTaskCompletionResult =
                serde_json::from_value(resp.result).expect("parse completion result");
            assert!(result.ok);
            assert_eq!(result.task.id, "loop-complete");
            assert_eq!(
                result.result.status,
                crate::loop_runtime::LoopCompletionStatus::Complete
            );
            assert!(result.result.reasons.is_empty());
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn evaluate_loop_task_completion_uses_projected_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    let task = crate::loop_runtime::LoopTaskRecord::new_for_test("loop-canceled", "ship")
        .with_completion_contract(crate::loop_runtime::LoopCompletionContract {
            required_checks: vec!["build:desktop".to_string()],
            max_gitnexus_risk: None,
            require_docs: false,
            require_commit: false,
            require_review_decision: false,
            stop_on_budget_exceeded: true,
        });
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::task_created(
            task, None, None,
        ))
        .unwrap();
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::task_canceled(
            "loop-canceled".to_string(),
            Some("stopped".to_string()),
            None,
            None,
        ))
        .unwrap();
    journal
        .append(crate::loop_runtime::LoopEventEnvelope::evidence_recorded(
            "loop-canceled".to_string(),
            crate::loop_runtime::EvidenceRecord::command_for_test("build:desktop", true),
            None,
            None,
        ))
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal, projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "evaluate-canceled-loop".into(),
            method: "evaluate_loop_task_completion".into(),
            params: Some(serde_json::json!({ "task_id": "loop-canceled" })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::EvaluateLoopTaskCompletionResult =
                serde_json::from_value(resp.result).expect("parse completion result");
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Canceled
            );
            assert!(result.task.evidence.is_empty());
            assert_ne!(
                result.result.status,
                crate::loop_runtime::LoopCompletionStatus::Complete
            );
            assert_eq!(
                result.result.reasons,
                vec!["missing_required_check:build:desktop"]
            );
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn evaluate_loop_task_completion_rejects_missing_task() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "evaluate-missing-loop".into(),
            method: "evaluate_loop_task_completion".into(),
            params: Some(serde_json::json!({ "task_id": "missing-loop" })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err
                .error
                .message
                .contains("loop task not found: missing-loop"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn cancel_loop_task_is_idempotent_through_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-cancel",
                "cancel me",
            ),
        )
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let request = || GatewayRequest {
        id: "cancel-loop".into(),
        method: "cancel_loop_task".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-cancel",
            "reason": "user canceled from dashboard"
        })),
    };

    let first = dispatch(&state, request());
    let second = dispatch(&state, request());

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::CancelLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse first cancel");
            assert!(result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Canceled
            );
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::CancelLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse second cancel");
            assert!(!result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Canceled
            );
        }
        _ => panic!("expected second Ok reply"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 2);
}

#[test]
fn cancel_loop_task_uses_stable_idempotency_key_fingerprint() {
    assert_eq!(
        cancel_idempotency_key("loop-1", Some("user canceled from dashboard")),
        "cancel:loop-1:e2baa3767ed51387"
    );
}

#[test]
fn recover_loop_task_marks_running_task_interrupted_and_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-recover",
                "recover me",
            ),
        )
        .unwrap();
    journal
        .append(loop_task_started_event(
            "loop-recover",
            "lease-recover",
            1,
            2,
        ))
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let request = || GatewayRequest {
        id: "recover-loop".into(),
        method: "recover_loop_task".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-recover",
            "reason": "stale lease recovered by operator"
        })),
    };

    let first = dispatch(&state, request());
    let second = dispatch(&state, request());

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse first recover");
            assert!(result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Interrupted
            );
            let recovery = result.task.recovery_state.expect("recovery state");
            assert_eq!(
                recovery.kind,
                crate::loop_runtime::LoopTaskRecoveryKind::Orphaned
            );
            assert!(recovery.recoverable);
            assert!(result.notice.contains("orphaned"));
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse second recover");
            assert!(!result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Interrupted
            );
        }
        _ => panic!("expected second Ok reply"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 3);
}

#[test]
fn recover_loop_task_export_evidence_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-evidence",
                "export evidence",
            ),
        )
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "recover-export".into(),
            method: "recover_loop_task".into(),
            params: Some(serde_json::json!({
                "task_id": "loop-evidence",
                "action": "export_evidence"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse export evidence");
            assert_eq!(
                result.action,
                crate::gateway::protocol::RecoveryActionKind::ExportEvidence
            );
            assert!(!result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Pending
            );
            let evidence = result.evidence.expect("recovery evidence");
            assert_eq!(evidence.task_id, "loop-evidence");
            assert_eq!(
                evidence.status,
                crate::loop_runtime::LoopTaskStatus::Pending
            );
            assert_eq!(evidence.event_count, 1);
        }
        _ => panic!("expected Ok reply"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 1);
}

#[test]
fn recover_loop_task_abandon_orphan_records_orphan_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-orphan",
                "abandon orphan",
            ),
        )
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "recover-orphan".into(),
            method: "recover_loop_task".into(),
            params: Some(serde_json::json!({
                "task_id": "loop-orphan",
                "action": "abandon_orphan"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse abandon orphan");
            assert_eq!(
                result.action,
                crate::gateway::protocol::RecoveryActionKind::AbandonOrphan
            );
            assert!(result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Interrupted
            );
            let recovery = result.task.recovery_state.expect("recovery state");
            assert_eq!(
                recovery.kind,
                crate::loop_runtime::LoopTaskRecoveryKind::Orphaned
            );
            assert!(recovery.reason.contains("orphaned"));
            assert!(result.notice.contains("orphaned"));
        }
        _ => panic!("expected Ok reply"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 2);
}

#[test]
fn recover_loop_task_retry_waiting_task_requeues_to_pending_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(crate::loop_runtime::LoopEventJournal::persistent_at(
        dir.path().join("loop-events.jsonl"),
    ));
    let projection = Arc::new(crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
        dir.path().join("loop-tasks.json"),
    ));
    journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-waiting",
                "retry safe waiting task",
            ),
        )
        .unwrap();
    journal
        .append(loop_task_waiting_for_input_event(
            "loop-waiting",
            "waiting for desktop owner",
            2,
        ))
        .unwrap();
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection);

    let request = || GatewayRequest {
        id: "recover-retry-waiting".into(),
        method: "recover_loop_task".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-waiting",
            "action": "retry_waiting_task",
            "reason": "operator requested safe retry"
        })),
    };

    let first = dispatch(&state, request());
    let second = dispatch(&state, request());

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse retry waiting result");
            assert_eq!(
                result.action,
                crate::gateway::protocol::RecoveryActionKind::RetryWaitingTask
            );
            assert!(result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Pending
            );
            assert!(result.task.outcome.is_none());
            assert!(result.notice.contains("requeued"));
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::RecoverLoopTaskResult =
                serde_json::from_value(resp.result).expect("parse idempotent retry waiting");
            assert!(!result.changed);
            assert_eq!(
                result.task.status,
                crate::loop_runtime::LoopTaskStatus::Pending
            );
        }
        _ => panic!("expected second Ok reply"),
    }
    assert_eq!(journal.load_all().unwrap().len(), 3);
}

#[test]
fn dispatch_list_trigger_runs_returns_records() {
    let state = test_gateway_state();
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-1".into(),
            trigger_id: "trigger-1".into(),
            session_id: None,
            attempt: 1,
            status: "completed".into(),
            message: "ledger ok".into(),
            started_at_ms: 1,
            ended_at_ms: 2,
            executor_kind: Some("eval_headless".into()),
            failure_category: Some("runner_error".into()),
            lease_expires_at_ms: Some(300_010),
            trigger_message: None,
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
        });

    let req = GatewayRequest {
        id: "runs".into(),
        method: "list_trigger_runs".into(),
        params: None,
    };
    let reply = dispatch(&state, req);

    match reply {
        GatewayReply::Ok(resp) => {
            let runs: Vec<crate::gateway::runner::TriggerRunRecord> =
                serde_json::from_value(resp.result).expect("parse trigger runs");
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].trigger_id, "trigger-1");
            assert_eq!(runs[0].status, "completed");
            assert_eq!(runs[0].executor_kind.as_deref(), Some("eval_headless"));
            assert_eq!(runs[0].failure_category.as_deref(), Some("runner_error"));
            assert_eq!(runs[0].lease_expires_at_ms, Some(300_010));
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn dispatch_runtime_status_returns_queue_and_run_summary() {
    let state = test_gateway_state();
    state.trigger_store.push(test_trigger("pending-1", None));
    state
        .trigger_store
        .push(test_trigger("claimed-1", Some(1234)));
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-pending",
                "pending loop task",
            ),
        )
        .unwrap();
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-running",
                "running loop task",
            ),
        )
        .unwrap();
    state
        .loop_event_journal
        .append(loop_task_started_event("loop-running", "lease-stale", 1, 2))
        .unwrap();
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-orphaned",
                "orphaned loop task",
            ),
        )
        .unwrap();
    state
        .loop_event_journal
        .append(task_interrupted_event(
            "loop-orphaned",
            "stale lease recovered by operator",
        ))
        .unwrap();
    state
        .loop_event_journal
        .append(headless_owner_run_requested_event(
            "loop-running",
            "lease-stale",
            1,
            "owner-waiting",
        ))
        .unwrap();
    state
        .loop_event_journal
        .append(headless_owner_run_state_event(
            "loop-running",
            "owner-run:loop-running:1:owner-waiting",
            "lease-stale",
            1,
            crate::loop_runtime::HeadlessOwnerRunState::WaitingForInput,
        ))
        .unwrap();
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-dead".into(),
            trigger_id: "claimed-1".into(),
            session_id: None,
            attempt: 3,
            status: "dead_letter".into(),
            message: "provider offline".into(),
            started_at_ms: 10,
            ended_at_ms: 11,
            executor_kind: Some("eval_headless".into()),
            failure_category: Some("runner_error".into()),
            lease_expires_at_ms: Some(300_010),
            trigger_message: None,
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
        });
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-ok".into(),
            trigger_id: "pending-1".into(),
            session_id: None,
            attempt: 1,
            status: "completed".into(),
            message: "ok".into(),
            started_at_ms: 20,
            ended_at_ms: 21,
            executor_kind: Some("eval_headless".into()),
            failure_category: None,
            lease_expires_at_ms: None,
            trigger_message: None,
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
        });
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 30,
        });
    state
        .session_input_store
        .complete_with_record("input-1")
        .expect("completion");

    let req = GatewayRequest {
        id: "runtime".into(),
        method: "runtime_status".into(),
        params: None,
    };
    let reply = dispatch(&state, req);

    match reply {
        GatewayReply::Ok(resp) => {
            assert_eq!(resp.result["dry_run_headless_owner_runs"], 1);
            assert_eq!(resp.result["waiting_headless_owner_runs"], 1);
            assert_eq!(resp.result["denied_headless_owner_runs"], 0);
            assert_eq!(resp.result["expired_headless_owner_runs"], 0);
            let status: GatewayRuntimeStatus =
                serde_json::from_value(resp.result).expect("parse runtime status");
            assert_eq!(status.pending_triggers, 1);
            assert_eq!(status.claimed_triggers, 1);
            assert_eq!(status.dead_letter_runs, 1);
            assert_eq!(
                status.ownership.ownership_mode,
                crate::gateway::protocol::GatewayOwnershipMode::LocalDefault
            );
            assert!(!status.ownership.gateway_default_enabled);
            assert!(!status.ownership.gateway_can_own_sessions);
            assert!(status.ownership.requires_opt_in);
            assert_eq!(status.ownership.parity_gate, "pending");
            assert_eq!(status.ownership.recovery_gate, "pending");
            assert!(!status.degraded_mode.active);
            assert_eq!(status.degraded_mode.fallback, "desktop_runtime");
            assert_eq!(status.pending_session_inputs, 0);
            assert_eq!(status.loop_runner, "stopped");
            assert_eq!(status.pending_loop_tasks, 1);
            assert_eq!(status.running_loop_tasks, 1);
            assert_eq!(status.stale_loop_task_leases, 1);
            assert_eq!(status.orphaned_loop_tasks, 1);
            assert_eq!(status.interrupted_loop_tasks, 0);
            assert_eq!(status.recoverable_loop_tasks, 1);
            assert_eq!(status.runtime_health.loop_tasks.pending, 1);
            assert_eq!(status.runtime_health.loop_tasks.running, 1);
            assert_eq!(status.runtime_health.loop_tasks.stale_leases, 1);
            assert_eq!(status.runtime_health.loop_tasks.orphaned, 1);
            assert_eq!(status.runtime_health.loop_tasks.recoverable, 1);
            assert_eq!(status.runtime_health.gateway_queue.pending_triggers, 1);
            assert_eq!(
                status.runtime_health.gateway_queue.pending_session_inputs,
                0
            );
            assert_eq!(status.dry_run_headless_owner_runs, 1);
            assert_eq!(status.waiting_headless_owner_runs, 1);
            assert_eq!(status.denied_headless_owner_runs, 0);
            assert_eq!(status.expired_headless_owner_runs, 0);
            assert_eq!(status.recent_runs.len(), 2);
            assert_eq!(status.recent_runs[0].id, "run-ok");
            assert_eq!(status.recent_runs[1].id, "run-dead");
            assert_eq!(
                status.recent_runs[1].executor_kind.as_deref(),
                Some("eval_headless")
            );
            assert_eq!(
                status.recent_runs[1].failure_category.as_deref(),
                Some("runner_error")
            );
            assert_eq!(status.recent_runs[1].lease_expires_at_ms, Some(300_010));
            assert_eq!(status.recent_session_inputs.len(), 1);
            assert_eq!(status.recent_session_inputs[0].input_id, "input-1");
            assert_eq!(status.recent_session_inputs[0].session_id, "session-1");
            assert_eq!(
                status
                    .runtime_tasks
                    .iter()
                    .map(|task| (task.name.as_str(), task.running))
                    .collect::<Vec<_>>(),
                [
                    (WEBHOOK_LISTENER_TASK, false),
                    (TRIGGER_RUNNER_TASK, false),
                    (LOOP_RUNNER_TASK, false),
                    (SCHEDULER_TICK_TASK, false),
                    (DASHBOARD_HTTP_TASK, false),
                ]
            );
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn gateway_local_parity_fixture_projects_backend_facts_without_taking_ownership() {
    #[derive(Debug, PartialEq, Eq)]
    struct BackendParityFacts {
        turn_prepared_summary: String,
        policy_decisions: Vec<String>,
        memory_audit: String,
        transcript_events: String,
        usage: String,
        runtime_projection: String,
        completion_evidence: String,
        ownership: String,
    }

    let state = test_gateway_state();
    state.register_session(test_session("session-1", "deepseek"));
    let task = crate::loop_runtime::LoopTaskRecord::new_for_test(
        "loop-parity",
        "fix visible button feedback",
    )
    .with_completion_contract(crate::loop_runtime::LoopCompletionContract {
        required_checks: vec!["build:desktop".to_string()],
        max_gitnexus_risk: None,
        require_docs: false,
        require_commit: false,
        require_review_decision: false,
        stop_on_budget_exceeded: true,
    });
    state
        .loop_event_journal
        .append(crate::loop_runtime::LoopEventEnvelope::task_created(
            task,
            Some("parity-create".into()),
            Some("parity:create".into()),
        ))
        .unwrap();
    state
        .loop_event_journal
        .append(policy_decision_event_for_test("loop-parity"))
        .unwrap();
    state
        .loop_event_journal
        .append(usage_ledger_event("loop-parity"))
        .unwrap();
    state
        .loop_event_journal
        .append(crate::loop_runtime::LoopEventEnvelope::evidence_recorded(
            "loop-parity".to_string(),
            crate::loop_runtime::EvidenceRecord::command_for_test("build:desktop", true),
            Some("parity-evidence".into()),
            Some("parity:evidence".into()),
        ))
        .unwrap();

    let enqueue = dispatch(
        &state,
        GatewayRequest {
            id: "parity-enqueue-input".into(),
            method: "enqueue_session_input".into(),
            params: Some(serde_json::json!({
                "input_id": "input-parity-1",
                "session_id": "session-1",
                "message": "continue with the minimal fix"
            })),
        },
    );
    assert!(matches!(enqueue, GatewayReply::Ok(_)));

    let listed_inputs = match dispatch(
        &state,
        GatewayRequest {
            id: "parity-list-input".into(),
            method: "list_session_inputs".into(),
            params: Some(serde_json::json!({
                "session_ids": ["session-1"],
                "limit": 8
            })),
        },
    ) {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::ListSessionInputsResult =
                serde_json::from_value(resp.result).expect("parse listed input");
            result.inputs
        }
        _ => panic!("expected list input Ok reply"),
    };
    let attach = match dispatch(
        &state,
        GatewayRequest {
            id: "parity-attach".into(),
            method: "attach_session".into(),
            params: Some(serde_json::json!({ "session_id": "session-1" })),
        },
    ) {
        GatewayReply::Ok(resp) => {
            serde_json::from_value::<crate::gateway::protocol::AttachSessionResult>(resp.result)
                .expect("parse attach")
        }
        _ => panic!("expected attach Ok reply"),
    };
    let dashboard = match dispatch(
        &state,
        GatewayRequest {
            id: "parity-dashboard".into(),
            method: "dashboard_snapshot".into(),
            params: None,
        },
    ) {
        GatewayReply::Ok(resp) => serde_json::from_value::<GatewayDashboardSnapshot>(resp.result)
            .expect("parse dashboard"),
        _ => panic!("expected dashboard Ok reply"),
    };
    let completion = match dispatch(
        &state,
        GatewayRequest {
            id: "parity-completion".into(),
            method: "evaluate_loop_task_completion".into(),
            params: Some(serde_json::json!({ "task_id": "loop-parity" })),
        },
    ) {
        GatewayReply::Ok(resp) => serde_json::from_value::<
            crate::gateway::protocol::EvaluateLoopTaskCompletionResult,
        >(resp.result)
        .expect("parse completion"),
        _ => panic!("expected completion Ok reply"),
    };

    let input = listed_inputs.first().expect("queued input");
    let task = dashboard
        .loop_tasks
        .iter()
        .find(|task| task.id == "loop-parity")
        .expect("projected loop task");
    let usage = task.latest_usage_ledger.as_ref().expect("usage ledger");
    let gateway_facts = BackendParityFacts {
        turn_prepared_summary: format!("{}:{}:{}", input.session_id, input.id, input.message),
        policy_decisions: task
            .policy_decisions
            .iter()
            .map(|decision| format!("{}:{}", decision.allowed, decision.reason))
            .collect(),
        memory_audit: if !dashboard.status.ownership.gateway_can_own_sessions {
            "desktop_owner_memory_audit".to_string()
        } else {
            "gateway_owner_memory_audit".to_string()
        },
        transcript_events: if attach.control.gateway_can_stream
            && !attach.control.gateway_can_own_session
        {
            "gateway_tail_read_only".to_string()
        } else {
            "gateway_tail_unavailable".to_string()
        },
        usage: format!(
            "provider={:?} model={:?} input={:?} output={:?} cost={:?}",
            usage.provider_id,
            usage.model,
            usage.input_tokens,
            usage.output_tokens,
            usage.estimated_cost_micros
        ),
        runtime_projection: format!(
            "pending={} running={} inputs={}",
            dashboard.status.pending_loop_tasks,
            dashboard.status.running_loop_tasks,
            dashboard.status.pending_session_inputs
        ),
        completion_evidence: format!(
            "{:?}:{:?}",
            completion.result.status, completion.result.reasons
        ),
        ownership: format!(
            "{:?}:{}:{}",
            dashboard.status.ownership.ownership_mode,
            dashboard.status.ownership.gateway_default_enabled,
            dashboard.status.ownership.gateway_can_own_sessions
        ),
    };
    let local_owner_facts = BackendParityFacts {
        turn_prepared_summary:
            "session-1:input-parity-1:continue with the minimal fix".to_string(),
        policy_decisions: vec!["true:allowed_by_background_task_policy".to_string()],
        memory_audit: "desktop_owner_memory_audit".to_string(),
        transcript_events: "gateway_tail_read_only".to_string(),
        usage:
            "provider=Some(\"deepseek\") model=Some(\"deepseek-v4-flash\") input=Some(111) output=Some(22) cost=Some(7)"
                .to_string(),
        runtime_projection: "pending=1 running=0 inputs=1".to_string(),
        completion_evidence: "Complete:[]".to_string(),
        ownership: "LocalDefault:false:false".to_string(),
    };

    assert_eq!(gateway_facts, local_owner_facts);
    assert!(!dashboard.status.ownership.gateway_default_enabled);
    assert!(!dashboard.status.ownership.gateway_can_own_sessions);
    assert_eq!(completion.task.evidence.len(), 1);
}

#[test]
fn gateway_ownership_eligibility_dry_run_denies_without_side_effects() {
    let state = test_gateway_state();
    state.register_session(test_session("session-1", "deepseek"));
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-gateway-owner",
                "summarize runtime status",
            ),
        )
        .unwrap();
    let enqueue = dispatch(
        &state,
        GatewayRequest {
            id: "eligibility-enqueue".into(),
            method: "enqueue_session_input".into(),
            params: Some(serde_json::json!({
                "input_id": "input-eligibility-1",
                "session_id": "session-1",
                "message": "summarize only"
            })),
        },
    );
    assert!(matches!(enqueue, GatewayReply::Ok(_)));
    let before_events = state.loop_event_journal.load_all().unwrap().len();
    let before_inputs = state.session_input_store.list().len();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "eligibility".into(),
            method: "evaluate_gateway_ownership_eligibility".into(),
            params: Some(serde_json::json!({
                "session_id": "session-1",
                "task_id": "loop-gateway-owner",
                "requested_mode": "gateway_read_only_owner"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GatewayOwnershipEligibilityResult =
                serde_json::from_value(resp.result).expect("parse eligibility result");
            assert!(result.ok);
            assert_eq!(
                result.decision,
                crate::gateway::protocol::GatewayOwnershipEligibilityDecision::Deny
            );
            assert_eq!(
                result.requested_mode,
                crate::gateway::protocol::GatewayOwnershipMode::GatewayReadOnlyOwner
            );
            assert_eq!(result.session_id.as_deref(), Some("session-1"));
            assert_eq!(result.task_id.as_deref(), Some("loop-gateway-owner"));
            assert!(result
                .reasons
                .contains(&"gateway_ownership_disabled".to_string()));
            assert!(result
                .missing_evidence
                .contains(&"memory_recall_audit".to_string()));
            assert!(result
                .missing_evidence
                .contains(&"permission_decision_ledger".to_string()));
            assert!(result
                .missing_evidence
                .contains(&"recovery_evidence".to_string()));
            assert!(!result.would_execute_provider);
            assert!(!result.would_execute_tools);
            assert!(!result.would_write_files);
            assert!(!result.changes_task_state);
        }
        _ => panic!("expected Ok reply"),
    }

    assert_eq!(
        state.loop_event_journal.load_all().unwrap().len(),
        before_events
    );
    assert_eq!(state.session_input_store.list().len(), before_inputs);
    let projection = state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
        .unwrap();
    let task = projection.find("loop-gateway-owner").expect("task");
    assert_eq!(task.status, crate::loop_runtime::LoopTaskStatus::Pending);
}

#[test]
fn gateway_patch_proposal_owner_gate_is_proposal_only_and_read_only() {
    let state = test_gateway_state();
    state.register_session(test_session("session-1", "deepseek"));
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-patch-proposal-owner",
                "propose a patch without applying it",
            ),
        )
        .unwrap();
    let before_events = state.loop_event_journal.load_all().unwrap().len();
    let before_inputs = state.session_input_store.list().len();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "patch-proposal-owner-gate".into(),
            method: "evaluate_gateway_ownership_eligibility".into(),
            params: Some(serde_json::json!({
                "session_id": "session-1",
                "task_id": "loop-patch-proposal-owner",
                "requested_mode": "gateway_patch_proposal_owner"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result = resp.result;
            assert_eq!(result["ok"], true);
            assert_eq!(result["decision"], "deny");
            assert_eq!(result["requested_mode"], "gateway_patch_proposal_owner");
            assert_eq!(result["proposal_only"], true);
            assert_eq!(result["would_generate_patch_proposal"], true);
            assert_eq!(result["would_apply_patch"], false);
            assert_eq!(result["would_write_files"], false);
            assert_eq!(result["would_execute_tools"], false);
            assert_eq!(result["changes_task_state"], false);
            let reasons = result["reasons"].as_array().expect("reasons array");
            assert!(reasons
                .iter()
                .any(|reason| reason == "gateway_ownership_disabled"));
            assert!(reasons
                .iter()
                .any(|reason| reason == "patch_proposal_owner_requires_gate"));
            let missing = result["missing_evidence"]
                .as_array()
                .expect("missing evidence array");
            assert!(missing
                .iter()
                .any(|evidence| evidence == "patch_proposal_review_gate"));
            assert!(missing
                .iter()
                .any(|evidence| evidence == "diff_evidence_contract"));
        }
        _ => panic!("expected Ok reply"),
    }

    assert_eq!(
        state.loop_event_journal.load_all().unwrap().len(),
        before_events
    );
    assert_eq!(state.session_input_store.list().len(), before_inputs);
    let projection = state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
        .unwrap();
    let task = projection.find("loop-patch-proposal-owner").expect("task");
    assert_eq!(task.status, crate::loop_runtime::LoopTaskStatus::Pending);
}

#[test]
fn gateway_read_only_owner_diagnostics_requires_explicit_allow_without_appending() {
    let state = test_gateway_state();
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-readonly-denied",
                "inspect projection only",
            ),
        )
        .unwrap();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "readonly-denied".into(),
            method: "run_gateway_read_only_owner_diagnostics".into(),
            params: Some(serde_json::json!({
                "task_id": "loop-readonly-denied",
                "session_id": "desktop-session-1",
                "requested_at_ms": 10,
                "expires_at_ms": 60_010,
                "idempotency_key": "readonly:loop-readonly-denied"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GatewayReadOnlyOwnerDiagnosticsResult =
                serde_json::from_value(resp.result).expect("parse readonly owner result");
            assert!(!result.ok);
            assert!(!result.started);
            assert!(!result.completed);
            assert!(!result.gateway_can_resume);
            assert!(result.owner_run.is_none());
            assert!(result.message.contains("requires explicit human approval"));
            assert_eq!(result.task.id, "loop-readonly-denied");
        }
        _ => panic!("expected Ok reply"),
    }
    assert_eq!(state.loop_event_journal.load_all().unwrap().len(), 1);
}

#[test]
fn gateway_read_only_owner_diagnostics_records_completed_owner_run_idempotently() {
    let state = test_gateway_state();
    state
        .loop_event_journal
        .append(
            crate::loop_runtime::LoopEventEnvelope::task_created_for_test(
                "loop-readonly-owner",
                "summarize projection",
            ),
        )
        .unwrap();

    let request = |id: &str| GatewayRequest {
        id: id.to_string(),
        method: "run_gateway_read_only_owner_diagnostics".into(),
        params: Some(serde_json::json!({
            "task_id": "loop-readonly-owner",
            "session_id": "desktop-session-1",
            "dev_only_allow": true,
            "requested_at_ms": 10,
            "expires_at_ms": 60_010,
            "idempotency_key": "readonly:loop-readonly-owner"
        })),
    };

    let first = dispatch(&state, request("readonly-owner-1"));
    let second = dispatch(&state, request("readonly-owner-2"));

    match first {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GatewayReadOnlyOwnerDiagnosticsResult =
                serde_json::from_value(resp.result).expect("parse readonly owner result");
            assert!(result.ok);
            assert!(result.started);
            assert!(result.completed);
            assert!(result.gateway_can_resume);
            assert!(result.summary.contains("Read-only diagnostics"));
            assert!(result
                .summary
                .contains("no provider, tool, shell, file, confirmation, or commit side effects"));
            assert!(!result.side_effects.provider);
            assert!(!result.side_effects.tools);
            assert!(!result.side_effects.shell);
            assert!(!result.side_effects.write_files);
            assert!(!result.side_effects.confirmations);
            assert!(!result.side_effects.commits);
            let owner_run = result.owner_run.expect("owner run");
            assert_eq!(
                owner_run.executor_kind,
                crate::loop_runtime::HeadlessOwnerExecutorKind::DryRun
            );
            assert_eq!(
                owner_run.state,
                crate::loop_runtime::HeadlessOwnerRunState::Completed
            );
            assert_eq!(owner_run.session_id.as_deref(), Some("desktop-session-1"));
            assert_eq!(owner_run.heartbeat_at_ms, Some(11));
            assert!(owner_run
                .evidence_refs
                .iter()
                .any(|evidence| evidence == "gateway_read_only_diagnostics"));
        }
        _ => panic!("expected first Ok reply"),
    }
    match second {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GatewayReadOnlyOwnerDiagnosticsResult =
                serde_json::from_value(resp.result).expect("parse duplicate readonly result");
            assert!(result.ok);
            assert!(result.completed);
            assert_eq!(
                result.owner_run.as_ref().unwrap().owner_run_id,
                "gateway-readonly-owner:loop-readonly-owner:1"
            );
        }
        _ => panic!("expected duplicate Ok reply"),
    }

    let events = state.loop_event_journal.load_all().unwrap();
    assert_eq!(events.len(), 4);
    let state_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.event,
                crate::loop_runtime::LoopRuntimeEvent::HeadlessOwnerRunStateRecorded { .. }
            )
        })
        .count();
    assert_eq!(state_events, 2);
    let projection = state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
        .expect("projection");
    let task = projection.find("loop-readonly-owner").expect("task");
    assert_eq!(task.headless_owner_runs.len(), 1);
    assert_eq!(
        task.headless_owner_runs[0].state,
        crate::loop_runtime::HeadlessOwnerRunState::Completed
    );
}

#[test]
fn dispatch_dashboard_snapshot_returns_dashboard_operational_summary() {
    let state = test_gateway_state();
    state.register_session(test_session("session-1", "claude"));
    state.trigger_store.push(test_trigger("pending-1", None));
    state
        .trigger_store
        .push(test_trigger("claimed-1", Some(1234)));
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-ok".into(),
            trigger_id: "pending-1".into(),
            session_id: Some("session-1".into()),
            attempt: 1,
            status: "completed".into(),
            message: "ok".into(),
            started_at_ms: 20,
            ended_at_ms: 21,
            executor_kind: Some("eval_headless".into()),
            failure_category: None,
            lease_expires_at_ms: Some(300_010),
            trigger_message: Some("run digest".into()),
            profile_id: Some("ops".into()),
            provider: Some("claude".into()),
            model: Some("sonnet".into()),
            workspace_path: Some("/repo".into()),
        });
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 30,
        });
    state
        .session_input_store
        .complete_with_record("input-1")
        .expect("completion");
    state
        .loop_event_journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-dashboard",
            "review runtime UI",
        ))
        .expect("append loop task");
    state
        .loop_event_journal
        .append(usage_ledger_event("loop-dashboard"))
        .expect("append usage ledger");
    state
        .loop_event_journal
        .append(headless_owner_run_requested_event(
            "loop-dashboard",
            "lease-dashboard",
            1,
            "owner-denied",
        ))
        .expect("append owner request");
    state
        .loop_event_journal
        .append(headless_owner_run_state_event(
            "loop-dashboard",
            "owner-run:loop-dashboard:1:owner-denied",
            "lease-dashboard",
            1,
            crate::loop_runtime::HeadlessOwnerRunState::Denied,
        ))
        .expect("append owner state");
    state.mark_runtime_task_failed(WEBHOOK_LISTENER_TASK, "address already in use");

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "dashboard".into(),
            method: "dashboard_snapshot".into(),
            params: None,
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let snapshot: GatewayDashboardSnapshot =
                serde_json::from_value(resp.result).expect("parse dashboard snapshot");
            assert!(snapshot.ok);
            assert!(snapshot.generated_at_ms > 0);
            assert_eq!(snapshot.status.loop_runner, "stopped");
            assert_eq!(snapshot.status.pending_triggers, 1);
            assert_eq!(snapshot.status.claimed_triggers, 1);
            assert_eq!(snapshot.status.pending_loop_tasks, 1);
            assert_eq!(snapshot.status.dry_run_headless_owner_runs, 1);
            assert_eq!(snapshot.status.waiting_headless_owner_runs, 0);
            assert_eq!(snapshot.status.denied_headless_owner_runs, 1);
            assert_eq!(snapshot.status.expired_headless_owner_runs, 0);
            assert_eq!(snapshot.loop_tasks.len(), 1);
            assert_eq!(snapshot.loop_tasks[0].id, "loop-dashboard");
            assert_eq!(
                snapshot.loop_tasks[0]
                    .latest_usage_ledger
                    .as_ref()
                    .and_then(|usage| usage.input_tokens),
                Some(111)
            );
            assert_eq!(snapshot.sessions.len(), 1);
            assert_eq!(snapshot.sessions[0].session_id, "session-1");
            assert_eq!(snapshot.queued_triggers.len(), 2);
            assert_eq!(snapshot.recent_runs.len(), 1);
            assert_eq!(snapshot.recent_runs[0].id, "run-ok");
            assert_eq!(
                snapshot.recent_runs[0].executor_kind.as_deref(),
                Some("eval_headless")
            );
            assert_eq!(snapshot.recent_runs[0].failure_category, None);
            assert_eq!(snapshot.recent_runs[0].lease_expires_at_ms, Some(300_010));
            assert_eq!(snapshot.recent_session_inputs.len(), 1);
            assert_eq!(snapshot.recent_session_inputs[0].input_id, "input-1");
            assert!(snapshot
                .event_log
                .iter()
                .any(|entry| entry.kind == "trigger_run" && entry.id == "run-ok"));
            assert!(snapshot
                .event_log
                .iter()
                .any(|entry| { entry.kind == "session_input_completed" && entry.id == "input-1" }));
            assert!(snapshot.event_log.iter().any(|entry| {
                entry.kind == "runtime_task_failed" && entry.id == WEBHOOK_LISTENER_TASK
            }));
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn dispatch_attach_session_classifies_session_states() {
    let state = test_gateway_state();
    state.register_session(test_session("session-live", "claude"));

    let mut restored = test_session("session-restored", "codex");
    restored.restored_from_registry = true;
    state.register_session(restored);

    let mut stale = test_session("session-stale", "openai");
    stale.last_seen_at_ms = Some(now_millis().saturating_sub(SESSION_STALE_AFTER_MS + 1));
    state.register_session(stale);

    let cases = [
        (
            "session-live",
            crate::gateway::protocol::GatewaySessionAttachStatus::Live,
            true,
        ),
        (
            "session-restored",
            crate::gateway::protocol::GatewaySessionAttachStatus::Restored,
            false,
        ),
        (
            "session-stale",
            crate::gateway::protocol::GatewaySessionAttachStatus::Stale,
            false,
        ),
        (
            "missing",
            crate::gateway::protocol::GatewaySessionAttachStatus::Missing,
            false,
        ),
    ];

    for (session_id, expected_status, expected_ok) in cases {
        let reply = dispatch(
            &state,
            GatewayRequest {
                id: format!("attach-{session_id}"),
                method: "attach_session".into(),
                params: Some(serde_json::json!({ "session_id": format!(" {session_id} ") })),
            },
        );

        match reply {
            GatewayReply::Ok(resp) => {
                let result: crate::gateway::protocol::AttachSessionResult =
                    serde_json::from_value(resp.result).expect("parse attach result");
                assert_eq!(result.session_id, session_id);
                assert_eq!(result.status, expected_status);
                assert_eq!(result.ok, expected_ok);
                assert_eq!(
                    result.control.gateway_can_stream,
                    expected_status == crate::gateway::protocol::GatewaySessionAttachStatus::Live
                );
                assert_eq!(
                    result.control.gateway_can_send_input,
                    expected_status == crate::gateway::protocol::GatewaySessionAttachStatus::Live
                );
                assert!(!result.control.gateway_can_resume);
                assert_eq!(
                    result.control.ownership_mode,
                    crate::gateway::protocol::GatewayOwnershipMode::LocalDefault
                );
                assert!(!result.control.gateway_can_own_session);
                assert!(!result.control.gateway_can_read_snapshot);
                assert_eq!(
                    result.control.control_plane,
                    match expected_status {
                        crate::gateway::protocol::GatewaySessionAttachStatus::Live =>
                            crate::gateway::protocol::GatewaySessionControlPlane::DesktopRuntimeRequired,
                        crate::gateway::protocol::GatewaySessionAttachStatus::Restored |
                        crate::gateway::protocol::GatewaySessionAttachStatus::Stale =>
                            crate::gateway::protocol::GatewaySessionControlPlane::DesktopRestoreRequired,
                        crate::gateway::protocol::GatewaySessionAttachStatus::Missing =>
                            crate::gateway::protocol::GatewaySessionControlPlane::Unavailable,
                    }
                );
                assert_eq!(
                    result.session.is_some(),
                    expected_status
                        != crate::gateway::protocol::GatewaySessionAttachStatus::Missing
                );
            }
            _ => panic!("expected Ok attach reply for {session_id}"),
        }
    }
}

#[test]
fn session_attach_control_reports_readable_snapshot_capability() {
    let control = session_attach_control(
        crate::gateway::protocol::GatewaySessionAttachStatus::Live,
        true,
    );

    assert_eq!(
        control.control_plane,
        crate::gateway::protocol::GatewaySessionControlPlane::DesktopRuntimeRequired
    );
    assert!(control.gateway_can_stream);
    assert!(control.gateway_can_send_input);
    assert_eq!(
        control.ownership_mode,
        crate::gateway::protocol::GatewayOwnershipMode::LocalDefault
    );
    assert!(!control.gateway_can_own_session);
    assert!(!control.gateway_can_resume);
    assert!(control.gateway_can_read_snapshot);
}

#[test]
fn session_attach_control_routes_missing_snapshot_to_restore_action() {
    let control = session_attach_control(
        crate::gateway::protocol::GatewaySessionAttachStatus::Missing,
        true,
    );

    assert_eq!(
        control.control_plane,
        crate::gateway::protocol::GatewaySessionControlPlane::DesktopRestoreRequired
    );
    assert!(control.gateway_can_stream);
    assert!(!control.gateway_can_own_session);
    assert!(control.gateway_can_read_snapshot);
    assert!(control.required_action.contains("snapshot"));
}

#[test]
fn runtime_status_reflects_background_task_state() {
    let state = test_gateway_state();
    state.mark_runtime_task_started(TRIGGER_RUNNER_TASK);
    state.mark_runtime_task_failed(WEBHOOK_LISTENER_TASK, "address already in use");

    let status = build_runtime_status(&state);
    let webhook = status
        .runtime_tasks
        .iter()
        .find(|task| task.name == WEBHOOK_LISTENER_TASK)
        .expect("webhook status");
    let trigger = status
        .runtime_tasks
        .iter()
        .find(|task| task.name == TRIGGER_RUNNER_TASK)
        .expect("trigger status");

    assert!(!webhook.running);
    assert_eq!(
        webhook.last_error.as_deref(),
        Some("address already in use")
    );
    assert!(trigger.running);
    assert!(trigger.last_started_at_ms.is_some());
    assert!(trigger.last_error.is_none());
    assert!(status.degraded_mode.active);
    assert!(status.degraded_mode.reason.contains("webhook_listener"));
    assert_eq!(status.degraded_mode.fallback, "desktop_runtime");
    assert_eq!(
        status.degraded_mode.recovery_command,
        "forge service restart"
    );
}

#[test]
fn default_runtime_tasks_include_dashboard_http() {
    let task_names = default_runtime_task_statuses()
        .into_iter()
        .map(|task| task.name)
        .collect::<Vec<_>>();

    assert!(task_names.contains(&DASHBOARD_HTTP_TASK.to_string()));
    assert!(task_names.contains(&LOOP_RUNNER_TASK.to_string()));
}

#[test]
fn dispatch_enqueue_trigger_pushes_to_store_and_updates_runtime_status() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "enqueue".into(),
            method: "enqueue_trigger".into(),
            params: Some(serde_json::json!({
                "trigger_id": "trigger-ipc-1",
                "message": "  run digest  ",
                "profile_id": "ops",
                "provider": "openai",
                "model": "gpt-5",
                "workspace_path": "/tmp/forge-workspace"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::EnqueueTriggerResult =
                serde_json::from_value(resp.result).expect("parse enqueue result");
            assert!(result.ok);
            assert_eq!(result.trigger_id, "trigger-ipc-1");
            assert_eq!(result.pending_triggers, 1);
        }
        _ => panic!("expected Ok reply"),
    }

    let queued = state.trigger_store.list();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, "trigger-ipc-1");
    assert_eq!(queued[0].message, "run digest");
    assert_eq!(queued[0].profile_id.as_deref(), Some("ops"));
    assert_eq!(queued[0].provider.as_deref(), Some("openai"));
    assert_eq!(queued[0].model.as_deref(), Some("gpt-5"));
    assert_eq!(
        queued[0].workspace_path.as_deref(),
        Some("/tmp/forge-workspace")
    );

    let status = build_runtime_status(&state);
    assert_eq!(status.pending_triggers, 1);
    assert_eq!(status.claimed_triggers, 0);
}

#[test]
fn dispatch_enqueue_session_input_pushes_to_inbox_and_updates_runtime_status() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "enqueue-input".into(),
            method: "enqueue_session_input".into(),
            params: Some(serde_json::json!({
                "input_id": "input-ipc-1",
                "session_id": " session-1 ",
                "message": " continue the work "
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::EnqueueSessionInputResult =
                serde_json::from_value(resp.result).expect("parse enqueue input result");
            assert!(result.ok);
            assert_eq!(result.input_id, "input-ipc-1");
            assert_eq!(result.session_id, "session-1");
            assert_eq!(result.pending_inputs, 1);
        }
        _ => panic!("expected Ok reply"),
    }

    let queued = state.session_input_store.list();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, "input-ipc-1");
    assert_eq!(queued[0].session_id, "session-1");
    assert_eq!(queued[0].message, "continue the work");

    let status = build_runtime_status(&state);
    assert_eq!(status.pending_session_inputs, 1);
}

#[test]
fn dispatch_enqueue_session_input_rejects_blank_message() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "enqueue-input".into(),
            method: "enqueue_session_input".into(),
            params: Some(serde_json::json!({
                "session_id": "session-1",
                "message": "   "
            })),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("message"));
        }
        _ => panic!("expected Err reply"),
    }
    assert!(state.session_input_store.list().is_empty());
}

#[test]
fn dispatch_list_session_inputs_filters_live_session_ids() {
    let state = test_gateway_state();
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-2".into(),
            session_id: "session-2".into(),
            message: "skip".into(),
            received_at_ms: 20,
        });
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 10,
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "list-inputs".into(),
            method: "list_session_inputs".into(),
            params: Some(serde_json::json!({
                "session_ids": [" session-1 ", "session-1"],
                "limit": 10
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::ListSessionInputsResult =
                serde_json::from_value(resp.result).expect("parse list input result");
            assert!(result.ok);
            assert_eq!(result.pending_inputs, 2);
            assert_eq!(result.inputs.len(), 1);
            assert_eq!(result.inputs[0].id, "input-1");
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn dispatch_complete_session_input_removes_record() {
    let state = test_gateway_state();
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 10,
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "complete-input".into(),
            method: "complete_session_input".into(),
            params: Some(serde_json::json!({
                "input_id": " input-1 "
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::CompleteSessionInputResult =
                serde_json::from_value(resp.result).expect("parse complete input result");
            assert!(result.ok);
            assert!(result.removed);
            assert_eq!(result.input_id, "input-1");
            assert_eq!(result.pending_inputs, 0);
        }
        _ => panic!("expected Ok reply"),
    }
    assert!(state.session_input_store.list().is_empty());
}

#[test]
fn dispatch_clear_stale_session_input_removes_record_with_recovery_evidence() {
    let state = test_gateway_state();
    state
        .session_input_store
        .push(crate::gateway::session_input::SessionInputRecord {
            id: "input-stale".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 10,
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "clear-stale-input".into(),
            method: "clear_stale_session_input".into(),
            params: Some(serde_json::json!({
                "input_id": " input-stale ",
                "reason": "operator cleared stale queued input"
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::ClearStaleSessionInputResult =
                serde_json::from_value(resp.result).expect("parse clear stale input result");
            assert!(result.ok);
            assert!(result.cleared);
            assert_eq!(result.input_id, "input-stale");
            assert_eq!(result.pending_inputs, 0);
            let evidence = result.evidence.expect("clear evidence");
            assert_eq!(
                evidence.action,
                crate::gateway::session_input::SessionInputCompletionAction::ClearedStale
            );
            assert_eq!(
                evidence.reason.as_deref(),
                Some("operator cleared stale queued input")
            );
        }
        _ => panic!("expected Ok reply"),
    }
    assert!(state.session_input_store.list().is_empty());
}

#[test]
fn dispatch_enqueue_trigger_rejects_blank_message() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "enqueue".into(),
            method: "enqueue_trigger".into(),
            params: Some(serde_json::json!({"message": "   "})),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("message"));
        }
        _ => panic!("expected Err reply"),
    }
    assert!(state.trigger_store.list().is_empty());
}

#[test]
fn dispatch_cancel_trigger_removes_pending_trigger() {
    let state = test_gateway_state();
    state
        .trigger_store
        .push(test_trigger("trigger-cancel", None));

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "cancel".into(),
            method: "cancel_trigger".into(),
            params: Some(serde_json::json!({"trigger_id": " trigger-cancel "})),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::CancelTriggerResult =
                serde_json::from_value(resp.result).expect("parse cancel result");
            assert!(result.ok);
            assert!(result.removed);
            assert_eq!(result.trigger_id, "trigger-cancel");
            assert_eq!(result.pending_triggers, 0);
        }
        _ => panic!("expected Ok reply"),
    }
    assert!(state.trigger_store.list().is_empty());
}

#[test]
fn dispatch_cancel_trigger_reports_missing_trigger_without_mutating_queue() {
    let state = test_gateway_state();
    state.trigger_store.push(test_trigger("trigger-keep", None));

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "cancel-missing".into(),
            method: "cancel_trigger".into(),
            params: Some(serde_json::json!({"trigger_id": "missing-trigger"})),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::CancelTriggerResult =
                serde_json::from_value(resp.result).expect("parse cancel result");
            assert!(result.ok);
            assert!(!result.removed);
            assert_eq!(result.trigger_id, "missing-trigger");
            assert_eq!(result.pending_triggers, 1);
        }
        _ => panic!("expected Ok reply"),
    }
    assert_eq!(state.trigger_store.list().len(), 1);
}

#[test]
fn dispatch_cancel_trigger_rejects_blank_id() {
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "cancel-blank".into(),
            method: "cancel_trigger".into(),
            params: Some(serde_json::json!({"trigger_id": "  "})),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("trigger_id"));
        }
        _ => panic!("expected Err reply"),
    }
}

#[test]
fn dispatch_replay_trigger_run_queues_new_trigger_from_run_metadata() {
    let state = test_gateway_state();
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-replayable".into(),
            trigger_id: "trigger-original".into(),
            session_id: None,
            attempt: 2,
            status: "dead_letter".into(),
            message: "provider offline".into(),
            started_at_ms: 10,
            ended_at_ms: 11,
            executor_kind: Some("eval_headless".into()),
            failure_category: Some("runner_error".into()),
            lease_expires_at_ms: None,
            trigger_message: Some("run the digest again".into()),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some("/repo/workspace".into()),
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "replay".into(),
            method: "replay_trigger_run".into(),
            params: Some(serde_json::json!({"run_id": " run-replayable "})),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::ReplayTriggerRunResult =
                serde_json::from_value(resp.result).expect("parse replay result");
            assert!(result.ok);
            assert_eq!(result.run_id, "run-replayable");
            assert_ne!(result.trigger_id, "trigger-original");
            assert_eq!(result.pending_triggers, 1);
        }
        _ => panic!("expected Ok reply"),
    }

    let queued = state.trigger_store.list();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].message, "run the digest again");
    assert_eq!(queued[0].profile_id.as_deref(), Some("ops"));
    assert_eq!(queued[0].provider.as_deref(), Some("openai"));
    assert_eq!(queued[0].model.as_deref(), Some("gpt-5"));
    assert_eq!(queued[0].workspace_path.as_deref(), Some("/repo/workspace"));
    assert_eq!(queued[0].attempt_count, 0);
    assert!(queued[0].claimed_at_ms.is_none());
}

#[test]
fn dispatch_replay_trigger_run_rejects_legacy_run_without_metadata() {
    let state = test_gateway_state();
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-legacy".into(),
            trigger_id: "trigger-legacy".into(),
            session_id: None,
            attempt: 1,
            status: "completed".into(),
            message: "old record".into(),
            started_at_ms: 10,
            ended_at_ms: 11,
            executor_kind: None,
            failure_category: None,
            lease_expires_at_ms: None,
            trigger_message: None,
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: None,
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "replay".into(),
            method: "replay_trigger_run".into(),
            params: Some(serde_json::json!({"run_id": "run-legacy"})),
        },
    );

    match reply {
        GatewayReply::Err(err) => {
            assert_eq!(err.error.code, -32602);
            assert!(err.error.message.contains("metadata"));
        }
        _ => panic!("expected Err reply"),
    }
    assert!(state.trigger_store.list().is_empty());
}

#[test]
fn dispatch_get_trigger_run_returns_requested_run_detail() {
    let state = test_gateway_state();
    state
        .trigger_run_store
        .push(crate::gateway::runner::TriggerRunRecord {
            id: "run-detail".into(),
            trigger_id: "trigger-detail".into(),
            session_id: None,
            attempt: 3,
            status: "dead_letter".into(),
            message: "provider offline".into(),
            started_at_ms: 10,
            ended_at_ms: 22,
            executor_kind: Some("eval_headless".into()),
            failure_category: Some("runner_error".into()),
            lease_expires_at_ms: Some(300_010),
            trigger_message: Some("run digest".into()),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some("/repo".into()),
        });

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "detail".into(),
            method: "get_trigger_run".into(),
            params: Some(serde_json::json!({"run_id": " run-detail "})),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GetTriggerRunResult =
                serde_json::from_value(resp.result).expect("parse detail result");
            assert!(result.ok);
            assert_eq!(result.run.id, "run-detail");
            assert_eq!(result.run.trigger_id, "trigger-detail");
            assert_eq!(result.run.executor_kind.as_deref(), Some("eval_headless"));
            assert_eq!(result.run.failure_category.as_deref(), Some("runner_error"));
            assert_eq!(result.run.lease_expires_at_ms, Some(300_010));
            assert_eq!(result.run.trigger_message.as_deref(), Some("run digest"));
            assert_eq!(result.run.workspace_path.as_deref(), Some("/repo"));
        }
        _ => panic!("expected Ok reply"),
    }
}

#[test]
fn dispatch_get_session_snapshot_returns_saved_snapshot_detail() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous_home = std::env::var("HOME").ok();
    let home = tempfile::tempdir().expect("home");
    std::env::set_var("HOME", home.path());
    let snapshot = crate::agent::snapshot::AgentSessionSnapshot::new(
        "snapshot-detail-session".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "/repo/detail".to_string(),
        vec![ChatMessage::user("show me")],
        Some("detail summary".to_string()),
        Some(128_000),
    );
    crate::agent::snapshot::save_session_snapshot(&snapshot).expect("save snapshot");
    let state = GatewayState::new();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "snapshot-detail".into(),
            method: "get_session_snapshot".into(),
            params: Some(serde_json::json!({"session_id": " snapshot-detail-session "})),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::GetSessionSnapshotResult =
                serde_json::from_value(resp.result).expect("parse snapshot result");
            assert!(result.ok);
            assert_eq!(result.session_id, "snapshot-detail-session");
            assert_eq!(result.snapshot["session_id"], "snapshot-detail-session");
            assert_eq!(result.snapshot["provider"], "deepseek");
            assert_eq!(result.snapshot["messages"][0]["content"], "show me");
        }
        _ => panic!("expected Ok reply"),
    }

    if let Some(value) = previous_home {
        std::env::set_var("HOME", value);
    } else {
        std::env::remove_var("HOME");
    }
}

#[test]
fn dispatch_tail_session_events_returns_transcript_cursor_window() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous_home = std::env::var("HOME").ok();
    let home = tempfile::tempdir().expect("home");
    std::env::set_var("HOME", home.path());
    crate::transcript::append_transcript_event(serde_json::json!({
        "event_type": "user_message",
        "session_id": "tail-session",
        "block_id": "user-1",
        "content": "hello"
    }))
    .expect("append first event");
    crate::transcript::append_transcript_event(serde_json::json!({
        "event_type": "text_chunk",
        "session_id": "tail-session",
        "block_id": "text-1",
        "content": "world"
    }))
    .expect("append second event");
    let state = test_gateway_state();

    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "tail-events".into(),
            method: "tail_session_events".into(),
            params: Some(serde_json::json!({
                "session_id": " tail-session ",
                "after_cursor": 1,
                "limit": 10
            })),
        },
    );

    match reply {
        GatewayReply::Ok(resp) => {
            let result: crate::gateway::protocol::TailSessionEventsResult =
                serde_json::from_value(resp.result).expect("parse tail result");
            assert!(result.ok);
            assert_eq!(result.session_id, "tail-session");
            assert_eq!(result.events.len(), 1);
            assert_eq!(result.events[0]["event_type"], "text_chunk");
            assert_eq!(result.total_events, 2);
            assert_eq!(result.next_cursor, 2);
            assert!(!result.cursor_reset);
        }
        _ => panic!("expected Ok reply"),
    }

    if let Some(value) = previous_home {
        std::env::set_var("HOME", value);
    } else {
        std::env::remove_var("HOME");
    }
}

// ── GatewayState ────────────────────────────────────────────────────

#[test]
fn gateway_state_starts_with_zero_sessions() {
    let state = test_gateway_state();
    assert_eq!(state.active_sessions(), 0);
}

#[test]
fn gateway_state_registers_and_unregisters_session_count() {
    let state = test_gateway_state();
    state.register_session(test_session("session-1", "claude"));
    assert_eq!(state.active_sessions(), 1);
    assert_eq!(state.list_sessions().len(), 1);

    state.unregister_session("session-1");
    assert_eq!(state.active_sessions(), 0);
    assert!(state.list_sessions().is_empty());
}

#[test]
fn gateway_state_replacing_session_does_not_double_count() {
    let state = test_gateway_state();
    state.register_session(test_session("session-1", "claude"));
    state.register_session(test_session("session-1", "codex"));

    let sessions = state.list_sessions();
    assert_eq!(state.active_sessions(), 1);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].provider, "codex");
}

#[test]
fn gateway_state_lists_snapshot_only_sessions_as_restored() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous_home = std::env::var("HOME").ok();
    let home = tempfile::tempdir().expect("home");
    std::env::set_var("HOME", home.path());
    let snapshot = crate::agent::snapshot::AgentSessionSnapshot::new(
        "snapshot-only-session".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "/repo/snapshot".to_string(),
        vec![ChatMessage::user("hello")],
        Some("snapshot summary".to_string()),
        Some(128_000),
    );
    crate::agent::snapshot::save_session_snapshot(&snapshot).expect("save snapshot");
    let state = GatewayState::new();

    let sessions = state.list_sessions();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "snapshot-only-session");
    assert_eq!(sessions[0].provider, "deepseek");
    assert_eq!(sessions[0].model, "deepseek-v4-flash");
    assert_eq!(sessions[0].workspace_path, "/repo/snapshot");
    assert!(sessions[0].restored_from_registry);
    assert_eq!(state.active_sessions(), 0);

    if let Some(value) = previous_home {
        std::env::set_var("HOME", value);
    } else {
        std::env::remove_var("HOME");
    }
}

#[test]
fn gateway_state_unregistering_missing_session_keeps_count_at_zero() {
    let state = test_gateway_state();
    state.unregister_session("missing-session");

    assert_eq!(state.active_sessions(), 0);
}

#[test]
fn active_session_count_excludes_stale_live_sessions() {
    let now_ms = SESSION_STALE_AFTER_MS + 10;
    let mut fresh = test_session("fresh-session", "claude");
    fresh.last_seen_at_ms = Some(now_ms);
    let mut stale = test_session("stale-session", "codex");
    stale.last_seen_at_ms = Some(1);
    let mut restored = test_session("restored-session", "openai");
    restored.last_seen_at_ms = Some(now_ms);
    restored.restored_from_registry = true;

    let sessions = HashMap::from([
        (fresh.session_id.clone(), fresh),
        (stale.session_id.clone(), stale),
        (restored.session_id.clone(), restored),
    ]);

    assert_eq!(active_session_count_at(&sessions, now_ms), 1);
}

#[test]
fn gateway_session_registry_restores_sessions_without_marking_them_active() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry_path = dir.path().join("gateway-sessions.json");
    let state = GatewayState::new_with_session_registry_path(registry_path.clone());
    state.register_session(test_session("session-1", "claude"));

    let restored = GatewayState::new_with_session_registry_path(registry_path.clone());
    let sessions = restored.list_sessions();

    assert_eq!(restored.active_sessions(), 0);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "session-1");
    assert_eq!(sessions[0].provider, "claude");
    assert!(sessions[0].restored_from_registry);

    restored.register_session(test_session("session-1", "claude"));
    let live_sessions = restored.list_sessions();
    assert_eq!(restored.active_sessions(), 1);
    assert!(!live_sessions[0].restored_from_registry);
}

#[test]
fn gateway_session_registry_persists_unregistered_sessions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let registry_path = dir.path().join("gateway-sessions.json");
    let state = GatewayState::new_with_session_registry_path(registry_path.clone());
    state.register_session(test_session("session-1", "claude"));
    state.unregister_session("session-1");

    let restored = GatewayState::new_with_session_registry_path(registry_path);

    assert_eq!(restored.active_sessions(), 0);
    assert!(restored.list_sessions().is_empty());
}

// ── default_socket_path ─────────────────────────────────────────────

#[test]
fn default_socket_path_ends_with_gateway_sock() {
    let path = default_socket_path();
    assert!(path.ends_with("gateway.sock"));
    assert!(path.to_string_lossy().contains(".forge"));
}

fn test_session(session_id: &str, provider: &str) -> GatewaySessionInfo {
    GatewaySessionInfo {
        session_id: session_id.to_string(),
        provider: provider.to_string(),
        model: "test-model".to_string(),
        workspace_path: "/tmp/forge-workspace".to_string(),
        created_at_ms: 1,
        owner_pid: Some(42),
        last_seen_at_ms: Some(now_millis()),
        restored_from_registry: false,
    }
}

fn test_trigger(id: &str, claimed_at_ms: Option<u64>) -> crate::gateway::webhook::PendingTrigger {
    crate::gateway::webhook::PendingTrigger {
        id: id.to_string(),
        message: "work".to_string(),
        profile_id: None,
        provider: None,
        model: None,
        workspace_path: None,
        attempt_count: 0,
        claimed_at_ms,
        received_at_ms: 1,
    }
}

fn loop_task_started_event(
    task_id: &str,
    lease_id: &str,
    acquired_at_ms: u64,
    expires_at_ms: u64,
) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-started"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::TaskStarted {
            task_id: task_id.to_string(),
            lease: crate::loop_runtime::LoopTaskLease {
                lease_id: lease_id.to_string(),
                owner_pid: 7,
                acquired_at_ms,
                expires_at_ms,
                heartbeat_at_ms: acquired_at_ms,
            },
        },
        actor: crate::loop_runtime::LoopActor::Runner {
            runner_id: "test-loop-runner".to_string(),
        },
        lease_id: Some(lease_id.to_string()),
        attempt: Some(1),
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: acquired_at_ms,
    }
}

fn loop_task_waiting_for_input_event(
    task_id: &str,
    reason: &str,
    waiting_at_ms: u64,
) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-waiting"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::TaskWaitingForInput {
            task_id: task_id.to_string(),
            reason: reason.to_string(),
            waiting_at_ms,
        },
        actor: crate::loop_runtime::LoopActor::Runner {
            runner_id: "test-loop-runner".to_string(),
        },
        lease_id: None,
        attempt: Some(1),
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: waiting_at_ms,
    }
}

fn usage_ledger_event(task_id: &str) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-usage"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::UsageLedgerRecorded {
            task_id: task_id.to_string(),
            usage: crate::loop_runtime::LoopUsageLedger {
                provider_id: Some("deepseek".to_string()),
                model: Some("deepseek-v4-flash".to_string()),
                input_tokens: Some(111),
                output_tokens: Some(22),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
                estimated_cost_micros: Some(7),
                pricing_source: Some("test".to_string()),
                has_unknown_input_tokens: false,
                has_unknown_output_tokens: false,
                has_unknown_cost: false,
                turn_count: 2,
                tool_call_count: 4,
                elapsed_ms: 3_000,
            },
        },
        actor: crate::loop_runtime::LoopActor::Gateway,
        lease_id: None,
        attempt: None,
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: 3,
    }
}

fn policy_decision_event_for_test(task_id: &str) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-policy"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::PolicyDecisionRecorded {
            task_id: task_id.to_string(),
            decision: crate::loop_runtime::PolicyDecisionRecord {
                decision_id: format!("policy-{task_id}"),
                intent: crate::loop_runtime::LoopActionIntent::ReadWorkspace {
                    path: "/repo".to_string(),
                },
                allowed: true,
                reason: "allowed_by_background_task_policy".to_string(),
                actor: crate::loop_runtime::LoopActor::Gateway,
                created_at_ms: 2,
            },
        },
        actor: crate::loop_runtime::LoopActor::Gateway,
        lease_id: None,
        attempt: None,
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: 2,
    }
}

fn task_interrupted_event(task_id: &str, reason: &str) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-interrupted"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::TaskInterrupted {
            task_id: task_id.to_string(),
            reason: reason.to_string(),
        },
        actor: crate::loop_runtime::LoopActor::Gateway,
        lease_id: None,
        attempt: None,
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: 4,
    }
}

fn headless_owner_run_requested_event(
    task_id: &str,
    lease_id: &str,
    attempt: u32,
    suffix: &str,
) -> crate::loop_runtime::LoopEventEnvelope {
    let owner_run_id = format!("owner-run:{task_id}:{attempt}:{suffix}");
    let idempotency_key = format!("owner-idempotency:{task_id}:{attempt}:{suffix}");
    let correlation_id = format!("owner-correlation:{task_id}:{attempt}:{suffix}");
    let owner_run = crate::loop_runtime::HeadlessOwnerRun {
        owner_run_id,
        task_id: task_id.to_string(),
        session_id: Some("desktop-session-1".to_string()),
        lease_id: lease_id.to_string(),
        attempt,
        state: crate::loop_runtime::HeadlessOwnerRunState::Requested,
        snapshot_source: crate::loop_runtime::HeadlessOwnerSnapshotSource::CurrentDesktopSession,
        snapshot_ref: Some("desktop-session-1".to_string()),
        human_gate_id: format!("event-{task_id}-approval"),
        policy_decision_id: format!("policy-{task_id}-{attempt}"),
        budget_snapshot_id: format!("event-{task_id}-budget"),
        idempotency_key,
        correlation_id,
        causation_id: Some(format!("event-{task_id}-budget")),
        requested_by: "runner:test-loop-runner".to_string(),
        requested_at_ms: 3,
        heartbeat_at_ms: None,
        expires_at_ms: 60_003,
        cancellation_reason: None,
        waiting_reason: None,
        executor_kind: crate::loop_runtime::HeadlessOwnerExecutorKind::DryRun,
        evidence_refs: vec![
            format!("event-{task_id}-approval"),
            format!("policy-{task_id}-{attempt}"),
            format!("event-{task_id}-budget"),
        ],
    };
    crate::loop_runtime::LoopEventEnvelope::headless_owner_run_requested(owner_run)
}

fn headless_owner_run_state_event(
    task_id: &str,
    owner_run_id: &str,
    lease_id: &str,
    attempt: u32,
    state: crate::loop_runtime::HeadlessOwnerRunState,
) -> crate::loop_runtime::LoopEventEnvelope {
    crate::loop_runtime::LoopEventEnvelope {
        schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-{owner_run_id}-{state:?}"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: crate::loop_runtime::LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
            task_id: task_id.to_string(),
            owner_run_id: owner_run_id.to_string(),
            state,
            heartbeat_at_ms: Some(4),
            cancellation_reason: None,
            waiting_reason: Some("test state".to_string()),
            evidence_refs: vec![format!("event-{task_id}-budget")],
        },
        actor: crate::loop_runtime::LoopActor::Runner {
            runner_id: "test-loop-runner".to_string(),
        },
        lease_id: Some(lease_id.to_string()),
        attempt: Some(attempt),
        correlation_id: Some(format!("owner-correlation:{task_id}:{attempt}")),
        causation_id: Some(format!("event-{task_id}-owner-requested")),
        idempotency_key: Some(format!("owner-state:{task_id}:{attempt}:{state:?}")),
        created_at_ms: 4,
    }
}
