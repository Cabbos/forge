use crate::loop_runtime::{
    HeadlessOwnerExecutorKind, HeadlessOwnerRun, HeadlessOwnerRunState,
    HeadlessOwnerSnapshotSource, HumanGateType, LoopActor, LoopEventEnvelope, LoopEventJournal,
    LoopRuntimeEvent, LoopTaskProjection, LoopTaskProjectionStore, LoopTaskRecoveryKind,
    LoopTaskStatus, LoopUsageLedger, LOOP_RUNTIME_SCHEMA_VERSION,
};
use serde_json::json;

#[test]
fn projection_rebuilds_after_projection_file_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let journal = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let projection = LoopTaskProjectionStore::persistent_at(temp.path().join("loop-tasks.json"));

    journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-1",
            "prove replay",
        ))
        .unwrap();
    std::fs::write(temp.path().join("loop-tasks.json"), "{broken").unwrap();

    let rebuilt = projection.load_or_rebuild(&journal).unwrap();

    assert_eq!(rebuilt.tasks[0].id, "loop-1");
}

#[test]
fn waiting_human_gate_survives_replay() {
    let events = vec![
        LoopEventEnvelope::task_created_for_test("loop-1", "install dependency"),
        LoopEventEnvelope::human_gate_requested_for_test(
            "loop-1",
            "gate-1",
            HumanGateType::PolicyOverride,
            "Approve dependency install",
        ),
    ];

    let projection = LoopTaskProjection::from_events(&events).unwrap();

    assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
    assert_eq!(projection.tasks[0].open_gates[0].gate_id, "gate-1");
}

#[test]
fn headless_owner_run_survives_journal_reload_rebuild_and_idempotent_retry() {
    let temp = tempfile::tempdir().unwrap();
    let journal = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let projection_store =
        LoopTaskProjectionStore::persistent_at(temp.path().join("loop-tasks.json"));
    journal
        .append_idempotent(
            LoopEventEnvelope::task_created_for_test("loop-owner", "headless owner")
                .with_idempotency_key("create:loop-owner"),
        )
        .unwrap();
    let first = headless_owner_run_requested_event_for_test(&owner_run_for_test(
        "owner-run-1",
        "loop-owner",
        "owner-idem-1",
    ));
    let retry = headless_owner_run_requested_event_for_test(&owner_run_for_test(
        "owner-run-regenerated",
        "loop-owner",
        "owner-idem-1",
    ));

    let first_result = journal.append_idempotent(first).unwrap();
    let retry_result = journal.append_idempotent(retry).unwrap();

    assert!(first_result.appended);
    assert!(!retry_result.appended);
    let reloaded = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let rebuilt = projection_store.load_or_rebuild(&reloaded).unwrap();

    assert_eq!(rebuilt.tasks.len(), 1);
    assert_eq!(rebuilt.tasks[0].headless_owner_runs.len(), 1);
    assert_eq!(
        rebuilt.tasks[0].headless_owner_runs[0].owner_run_id,
        "owner-run-1"
    );
    assert_eq!(
        rebuilt.tasks[0].headless_owner_runs[0].idempotency_key,
        "owner-idem-1"
    );
}

#[test]
fn recovery_and_usage_survive_journal_reload_rebuild() {
    let temp = tempfile::tempdir().unwrap();
    let journal = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let projection_store =
        LoopTaskProjectionStore::persistent_at(temp.path().join("loop-tasks.json"));
    journal
        .append(LoopEventEnvelope::task_created_for_test(
            "loop-recovery",
            "recover stale loop",
        ))
        .unwrap();
    journal
        .append(usage_ledger_event_for_test("loop-recovery"))
        .unwrap();
    journal
        .append(task_interrupted_event_for_test(
            "loop-recovery",
            "stale lease recovered by operator",
        ))
        .unwrap();

    let reloaded = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let rebuilt = projection_store.load_or_rebuild(&reloaded).unwrap();
    let task = rebuilt.find("loop-recovery").expect("projected task");

    assert_eq!(task.status, LoopTaskStatus::Interrupted);
    assert_eq!(
        task.latest_usage_ledger
            .as_ref()
            .and_then(|usage| usage.input_tokens),
        Some(123)
    );
    let recovery = task.recovery_state.as_ref().expect("recovery state");
    assert_eq!(recovery.kind, LoopTaskRecoveryKind::Orphaned);
    assert!(recovery.recoverable);
    assert!(recovery.reason.contains("stale lease"));
    assert!(task
        .outcome
        .as_ref()
        .unwrap()
        .message
        .contains("stale lease"));
}

fn owner_run_for_test(
    owner_run_id: &str,
    task_id: &str,
    idempotency_key: &str,
) -> HeadlessOwnerRun {
    HeadlessOwnerRun {
        owner_run_id: owner_run_id.to_string(),
        task_id: task_id.to_string(),
        session_id: Some("session-owner".to_string()),
        lease_id: "lease-owner-1".to_string(),
        attempt: 1,
        state: HeadlessOwnerRunState::Requested,
        snapshot_source: HeadlessOwnerSnapshotSource::PersistedSessionSnapshot,
        snapshot_ref: Some("snapshot-owner-1".to_string()),
        human_gate_id: "gate-owner-1".to_string(),
        policy_decision_id: "policy-owner-1".to_string(),
        budget_snapshot_id: "budget-owner-1".to_string(),
        idempotency_key: idempotency_key.to_string(),
        correlation_id: "corr-owner-1".to_string(),
        causation_id: Some("cause-owner-1".to_string()),
        requested_by: "controller".to_string(),
        requested_at_ms: 1_000,
        heartbeat_at_ms: None,
        expires_at_ms: 2_000,
        cancellation_reason: None,
        waiting_reason: None,
        executor_kind: HeadlessOwnerExecutorKind::DryRun,
        evidence_refs: vec!["request-evidence".to_string()],
    }
}

fn headless_owner_run_requested_event_for_test(owner_run: &HeadlessOwnerRun) -> LoopEventEnvelope {
    serde_json::from_value(json!({
        "schema_version": LOOP_RUNTIME_SCHEMA_VERSION,
        "event_id": format!("event-{}-{}-requested", owner_run.task_id, owner_run.owner_run_id),
        "task_id": owner_run.task_id,
        "sequence": 0,
        "event": {
            "type": "headless_owner_run_requested",
            "task_id": owner_run.task_id,
            "owner_run": owner_run
        },
        "actor": { "kind": "gateway" },
        "lease_id": owner_run.lease_id,
        "attempt": owner_run.attempt,
        "correlation_id": owner_run.correlation_id,
        "causation_id": owner_run.causation_id,
        "idempotency_key": owner_run.idempotency_key,
        "created_at_ms": owner_run.requested_at_ms
    }))
    .unwrap()
}

fn usage_ledger_event_for_test(task_id: &str) -> LoopEventEnvelope {
    LoopEventEnvelope {
        schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-usage"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: LoopRuntimeEvent::UsageLedgerRecorded {
            task_id: task_id.to_string(),
            usage: LoopUsageLedger {
                provider_id: Some("deepseek".to_string()),
                model: Some("deepseek-v4-flash".to_string()),
                input_tokens: Some(123),
                output_tokens: Some(45),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
                estimated_cost_micros: Some(9),
                pricing_source: Some("test".to_string()),
                has_unknown_input_tokens: false,
                has_unknown_output_tokens: false,
                has_unknown_cost: false,
                turn_count: 2,
                tool_call_count: 3,
                elapsed_ms: 4_000,
            },
        },
        actor: LoopActor::Gateway,
        lease_id: None,
        attempt: None,
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: 3,
    }
}

fn task_interrupted_event_for_test(task_id: &str, reason: &str) -> LoopEventEnvelope {
    LoopEventEnvelope {
        schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: format!("event-{task_id}-interrupted"),
        task_id: task_id.to_string(),
        sequence: 0,
        event: LoopRuntimeEvent::TaskInterrupted {
            task_id: task_id.to_string(),
            reason: reason.to_string(),
        },
        actor: LoopActor::Gateway,
        lease_id: None,
        attempt: None,
        correlation_id: None,
        causation_id: None,
        idempotency_key: None,
        created_at_ms: 4,
    }
}
