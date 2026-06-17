use crate::loop_runtime::{
    HumanGateType, LoopEventEnvelope, LoopEventJournal, LoopTaskProjection,
    LoopTaskProjectionStore, LoopTaskStatus,
};

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
