use std::collections::BTreeMap;
use std::sync::Arc;

use crate::agent::a2a::bus::{AgentA2ABus, AgentReviewDecision};
use crate::agent::a2a::ledger::{self, AgentA2ALedgerLoadError, AgentA2AProjectionList};
use crate::agent::a2a::projection::AgentA2AProjection;
use crate::agent::a2a::types::AgentTaskId;
use crate::agent::event_sink::TauriEventEmitter;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::snapshot::{load_session_snapshot, save_session_snapshot};
use crate::agent::time::now_ms;
use crate::ipc::session_lifecycle::save_session_snapshot_with_workflow;
use crate::loop_runtime::{
    EvidenceRecord, HumanGateDecision, HumanGateDecisionKind, HumanGateType, LoopEventEnvelope,
    LoopEventJournal, LoopTaskProjectionStore,
};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentA2AStateSource {
    Live,
    Ledger,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ASessionState {
    pub session_id: String,
    pub source: AgentA2AStateSource,
    pub state: AgentA2AProjection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2AStatesPayload {
    pub states: Vec<AgentA2ASessionState>,
    pub load_errors: Vec<AgentA2ALedgerLoadError>,
}

#[tauri::command]
pub async fn get_agent_a2a_state(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Option<AgentA2ASessionState>, String> {
    let live_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = live_session {
        return Ok(Some(live_session_state(&session_id, &session)));
    }

    Ok(
        ledger::load_session_projection(&session_id)?.map(|projection| AgentA2ASessionState {
            session_id,
            source: AgentA2AStateSource::Ledger,
            state: projection,
        }),
    )
}

#[tauri::command]
pub async fn list_agent_a2a_states(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<AgentA2AStatesPayload, String> {
    let live_states = state
        .sessions
        .read()
        .await
        .iter()
        .map(|(session_id, session)| live_session_state(session_id, session))
        .collect::<Vec<_>>();
    let ledger_states = ledger::list_session_projections()?;

    Ok(merge_a2a_state_sources(ledger_states, live_states))
}

#[tauri::command]
pub async fn review_agent_a2a_tasks(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    task_ids: Vec<String>,
    decision: AgentReviewDecision,
    message: Option<String>,
    loop_task_id: Option<String>,
) -> Result<AgentA2ASessionState, String> {
    let task_ids = normalize_review_task_ids(task_ids)?;
    let message = message.unwrap_or_default();
    let loop_task_id = clean_optional_string(loop_task_id);
    let timestamp_ms = now_ms();
    let live_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = live_session {
        {
            let mut bus = lock_unpoisoned(&session.a2a_bus);
            apply_review_decisions(&mut bus, &task_ids, decision, &message, timestamp_ms)?;
        }
        if let Err(error) = save_session_snapshot_with_workflow(&state, &session).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
        record_loop_review_decision_with_persistent_stores(
            &session_id,
            loop_task_id.as_deref(),
            decision,
            &message,
            timestamp_ms,
        );
        session.emit_a2a_projection(&TauriEventEmitter::new(app_handle));
        return Ok(live_session_state(&session_id, &session));
    }

    let mut snapshot = match load_session_snapshot(&session_id) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            crate::app_log!("WARN", "[a2a_review] snapshot load skipped: {}", error);
            None
        }
    };
    let mut bus = if let Some(bus) = snapshot
        .as_mut()
        .and_then(|snapshot| snapshot.a2a_state.take())
    {
        bus
    } else {
        ledger::load_session_ledger(&session_id)?
            .ok_or_else(|| format!("A2A state not found for session '{session_id}'"))?
    };
    apply_review_decisions(&mut bus, &task_ids, decision, &message, timestamp_ms)?;
    let projection = bus.projection();
    if let Some(snapshot) = snapshot {
        save_session_snapshot(&snapshot.with_a2a_state(bus))?;
    } else {
        ledger::save_session_ledger(&session_id, &bus)?;
    }
    record_loop_review_decision_with_persistent_stores(
        &session_id,
        loop_task_id.as_deref(),
        decision,
        &message,
        timestamp_ms,
    );

    Ok(AgentA2ASessionState {
        session_id,
        source: AgentA2AStateSource::Ledger,
        state: projection,
    })
}

fn live_session_state(session_id: &str, session: &Arc<AgentSession>) -> AgentA2ASessionState {
    AgentA2ASessionState {
        session_id: session_id.to_string(),
        source: AgentA2AStateSource::Live,
        state: session.a2a_projection(),
    }
}

fn normalize_review_task_ids(task_ids: Vec<String>) -> Result<Vec<AgentTaskId>, String> {
    let mut normalized = Vec::new();
    for raw in task_ids {
        let task_id = raw.trim();
        if task_id.is_empty() {
            continue;
        }
        if !normalized
            .iter()
            .any(|existing: &AgentTaskId| existing.as_str() == task_id)
        {
            normalized.push(AgentTaskId::new(task_id));
        }
    }
    if normalized.is_empty() {
        return Err("At least one task id is required".to_string());
    }
    Ok(normalized)
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn apply_review_decisions(
    bus: &mut AgentA2ABus,
    task_ids: &[AgentTaskId],
    decision: AgentReviewDecision,
    message: &str,
    timestamp_ms: u64,
) -> Result<(), String> {
    let mut next = bus.clone();
    for task_id in task_ids {
        next.record_review_decision(task_id, decision, message.to_string(), timestamp_ms)?;
    }
    *bus = next;
    Ok(())
}

fn record_loop_review_decision_with_persistent_stores(
    session_id: &str,
    loop_task_id: Option<&str>,
    decision: AgentReviewDecision,
    message: &str,
    timestamp_ms: u64,
) -> bool {
    let journal = LoopEventJournal::persistent_default();
    let projection = LoopTaskProjectionStore::persistent_default();
    record_loop_review_decision_for_a2a_best_effort(
        &journal,
        &projection,
        session_id,
        loop_task_id,
        decision,
        message,
        timestamp_ms,
    )
}

fn record_loop_review_decision_for_a2a_best_effort(
    journal: &LoopEventJournal,
    projection_store: &LoopTaskProjectionStore,
    session_id: &str,
    loop_task_id: Option<&str>,
    decision: AgentReviewDecision,
    message: &str,
    timestamp_ms: u64,
) -> bool {
    match record_loop_review_decision_for_a2a(
        journal,
        projection_store,
        session_id,
        loop_task_id,
        decision,
        message,
        timestamp_ms,
    ) {
        Ok(recorded) => recorded,
        Err(error) => {
            crate::app_log!("WARN", "[a2a_review] loop bridge skipped: {}", error);
            false
        }
    }
}

fn record_loop_review_decision_for_a2a(
    journal: &LoopEventJournal,
    projection_store: &LoopTaskProjectionStore,
    session_id: &str,
    loop_task_id: Option<&str>,
    decision: AgentReviewDecision,
    message: &str,
    timestamp_ms: u64,
) -> Result<bool, String> {
    let Some(loop_task_id) = loop_task_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Ok(false);
    };
    let projection = projection_store.load_or_rebuild(journal)?;
    let Some(task) = projection.find(loop_task_id) else {
        return Ok(false);
    };
    if task.session_id.as_deref() != Some(session_id) {
        return Ok(false);
    }

    let gate_id = task
        .open_gates
        .first()
        .map(|gate| gate.gate_id.clone())
        .unwrap_or_else(|| format!("a2a-review-{loop_task_id}"));
    let reason = review_reason(decision, message);
    let human_decision = HumanGateDecision {
        kind: match decision {
            AgentReviewDecision::Approve => HumanGateDecisionKind::Approved,
            AgentReviewDecision::Reject => HumanGateDecisionKind::Denied,
        },
        decided_at_ms: timestamp_ms,
        decided_by: Some("a2a_review".to_string()),
        reason,
    };
    let decision_key = review_decision_key(decision, human_decision.reason.as_deref());

    if !task.open_gates.iter().any(|gate| gate.gate_id == gate_id) {
        journal.append_idempotent(LoopEventEnvelope::human_gate_requested(
            loop_task_id.to_string(),
            gate_id.clone(),
            HumanGateType::PolicyOverride,
            "A2A review decision for loop task".to_string(),
            None,
            Some(format!(
                "a2a_review_gate_requested:{loop_task_id}:{gate_id}"
            )),
        ))?;
    }
    journal.append_idempotent(LoopEventEnvelope::human_gate_resolved(
        loop_task_id.to_string(),
        gate_id.clone(),
        human_decision.clone(),
        None,
        Some(format!(
            "a2a_review_gate_resolved:{loop_task_id}:{gate_id}:{decision_key}"
        )),
    ))?;
    journal.append_idempotent(LoopEventEnvelope::evidence_recorded(
        loop_task_id.to_string(),
        EvidenceRecord::Review {
            evidence_id: format!("evidence-a2a-review-{loop_task_id}-{gate_id}-{decision_key}"),
            gate_id: gate_id.clone(),
            decision: human_decision,
        },
        None,
        Some(format!(
            "a2a_review_evidence:{loop_task_id}:{gate_id}:{decision_key}"
        )),
    ))?;
    projection_store.rebuild_from_journal(journal)?;

    Ok(true)
}

fn review_reason(decision: AgentReviewDecision, message: &str) -> Option<String> {
    let trimmed = message.trim();
    if !trimmed.is_empty() {
        return Some(trimmed.to_string());
    }
    match decision {
        AgentReviewDecision::Approve => None,
        AgentReviewDecision::Reject => Some("review rejected".to_string()),
    }
}

fn review_decision_key(decision: AgentReviewDecision, reason: Option<&str>) -> String {
    let decision = match decision {
        AgentReviewDecision::Approve => "approved",
        AgentReviewDecision::Reject => "denied",
    };
    match reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("{decision}:{}", stable_text_hash(reason)),
        None => decision.to_string(),
    }
}

fn stable_text_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn merge_a2a_state_sources(
    ledger: AgentA2AProjectionList,
    live: Vec<AgentA2ASessionState>,
) -> AgentA2AStatesPayload {
    let mut states = BTreeMap::new();
    for item in ledger.states {
        states.insert(
            item.session_id.clone(),
            AgentA2ASessionState {
                session_id: item.session_id,
                source: AgentA2AStateSource::Ledger,
                state: item.projection,
            },
        );
    }
    for item in live {
        states.insert(item.session_id.clone(), item);
    }

    AgentA2AStatesPayload {
        states: states.into_values().collect(),
        load_errors: ledger.load_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::ledger::{
        AgentA2ALedgerLoadError, AgentA2AProjectionList, AgentA2ASessionProjection,
    };
    use crate::agent::a2a::projection::AgentA2AProjection;

    #[test]
    fn merge_a2a_state_sources_prefers_live_state_over_ledger() {
        let ledger_projection = AgentA2AProjection {
            completed_count: 1,
            ..AgentA2AProjection::default()
        };
        let live_projection = AgentA2AProjection {
            running_count: 1,
            ..AgentA2AProjection::default()
        };
        let ledger = AgentA2AProjectionList {
            states: vec![AgentA2ASessionProjection {
                session_id: "session-1".to_string(),
                projection: ledger_projection,
            }],
            load_errors: Vec::new(),
        };
        let live = vec![AgentA2ASessionState {
            session_id: "session-1".to_string(),
            source: AgentA2AStateSource::Live,
            state: live_projection,
        }];

        let payload = merge_a2a_state_sources(ledger, live);

        assert_eq!(payload.states.len(), 1);
        assert_eq!(payload.states[0].source, AgentA2AStateSource::Live);
        assert_eq!(payload.states[0].state.running_count, 1);
    }

    #[test]
    fn merge_a2a_state_sources_keeps_ledger_load_errors() {
        let ledger = AgentA2AProjectionList {
            states: Vec::new(),
            load_errors: vec![AgentA2ALedgerLoadError {
                session_id: "session-bad".to_string(),
                message: "parse failed".to_string(),
            }],
        };

        let payload = merge_a2a_state_sources(ledger, Vec::new());

        assert!(payload.states.is_empty());
        assert_eq!(payload.load_errors.len(), 1);
        assert_eq!(payload.load_errors[0].session_id, "session-bad");
    }

    #[test]
    fn normalize_review_task_ids_trims_and_dedupes() {
        let task_ids = normalize_review_task_ids(vec![
            " review-task-1 ".to_string(),
            "".to_string(),
            "review-task-1".to_string(),
            "review-task-2".to_string(),
        ])
        .expect("task ids");

        assert_eq!(
            task_ids
                .iter()
                .map(|task_id| task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["review-task-1", "review-task-2"]
        );
    }

    #[test]
    fn apply_review_decisions_is_all_or_nothing() {
        use crate::agent::a2a::bus::{AgentA2ABus, AgentReviewDecision};
        use crate::agent::a2a::types::{
            AgentArtifact, AgentArtifactKind, AgentExecutionMode, AgentRole, AgentTaskId,
        };

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Reviewable task",
            "Do work",
            10,
        );
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "needs_human_review": true,
                    "worktree_path": "/tmp/reviewable",
                })
                .to_string(),
                created_at_ms: 20,
            },
            20,
        );
        bus.complete_task(&task_id, "Ready", 30);

        let result = apply_review_decisions(
            &mut bus,
            &[task_id.clone(), AgentTaskId::new("missing-task")],
            AgentReviewDecision::Approve,
            "",
            40,
        );

        assert!(result.is_err());
        let projection = bus.projection();
        assert_eq!(projection.tasks[0].needs_human_review, Some(true));
        assert_eq!(projection.tasks[0].review_decision, None);
    }

    #[test]
    fn loop_review_bridge_records_only_matching_loop_task_session() {
        let temp = tempfile::tempdir().unwrap();
        let journal = crate::loop_runtime::LoopEventJournal::persistent_at(
            temp.path().join("loop-events.jsonl"),
        );
        let projection = crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
            temp.path().join("loop-tasks.json"),
        );
        let mut task = crate::loop_runtime::LoopTaskRecord::new_for_test("loop-1", "ship");
        task.session_id = Some("session-1".to_string());
        journal
            .append(crate::loop_runtime::LoopEventEnvelope::task_created(
                task, None, None,
            ))
            .unwrap();

        let recorded = record_loop_review_decision_for_a2a(
            &journal,
            &projection,
            "session-1",
            Some("loop-1"),
            AgentReviewDecision::Reject,
            "needs tests",
            42,
        )
        .expect("bridge result");

        assert!(recorded);
        let replayed = projection.load_or_rebuild(&journal).unwrap();
        let task = replayed.find("loop-1").expect("loop task");
        let review = task
            .evidence
            .iter()
            .find_map(|evidence| match evidence {
                crate::loop_runtime::EvidenceRecord::Review {
                    gate_id, decision, ..
                } => Some((gate_id, decision)),
                _ => None,
            })
            .expect("review evidence");
        assert_eq!(review.0, "a2a-review-loop-1");
        assert_eq!(
            review.1.kind,
            crate::loop_runtime::HumanGateDecisionKind::Denied
        );
        assert_eq!(review.1.reason.as_deref(), Some("needs tests"));
    }

    #[test]
    fn loop_review_bridge_retries_use_stable_idempotency_keys() {
        let temp = tempfile::tempdir().unwrap();
        let journal = crate::loop_runtime::LoopEventJournal::persistent_at(
            temp.path().join("loop-events.jsonl"),
        );
        let projection = crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
            temp.path().join("loop-tasks.json"),
        );
        let mut task = crate::loop_runtime::LoopTaskRecord::new_for_test("loop-1", "ship");
        task.session_id = Some("session-1".to_string());
        journal
            .append(crate::loop_runtime::LoopEventEnvelope::task_created(
                task, None, None,
            ))
            .unwrap();

        let first = record_loop_review_decision_for_a2a(
            &journal,
            &projection,
            "session-1",
            Some("loop-1"),
            AgentReviewDecision::Reject,
            "needs tests",
            42,
        )
        .expect("first bridge result");
        let second = record_loop_review_decision_for_a2a(
            &journal,
            &projection,
            "session-1",
            Some("loop-1"),
            AgentReviewDecision::Reject,
            "needs tests",
            43,
        )
        .expect("second bridge result");

        assert!(first);
        assert!(second);
        let replayed = projection.load_or_rebuild(&journal).unwrap();
        let task = replayed.find("loop-1").expect("loop task");
        let review_evidence = task
            .evidence
            .iter()
            .filter(|evidence| {
                matches!(evidence, crate::loop_runtime::EvidenceRecord::Review { .. })
            })
            .count();
        assert_eq!(review_evidence, 1);
        assert_eq!(journal.load_all().unwrap().len(), 4);
    }

    #[test]
    fn loop_review_bridge_best_effort_returns_false_on_journal_error() {
        let temp = tempfile::tempdir().unwrap();
        let journal =
            crate::loop_runtime::LoopEventJournal::persistent_at(temp.path().to_path_buf());
        let projection = crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
            temp.path().join("loop-tasks.json"),
        );

        let recorded = record_loop_review_decision_for_a2a_best_effort(
            &journal,
            &projection,
            "session-1",
            Some("loop-1"),
            AgentReviewDecision::Approve,
            "",
            42,
        );

        assert!(!recorded);
    }

    #[test]
    fn loop_review_bridge_skips_mismatched_or_missing_loop_task() {
        let temp = tempfile::tempdir().unwrap();
        let journal = crate::loop_runtime::LoopEventJournal::persistent_at(
            temp.path().join("loop-events.jsonl"),
        );
        let projection = crate::loop_runtime::LoopTaskProjectionStore::persistent_at(
            temp.path().join("loop-tasks.json"),
        );
        let mut task = crate::loop_runtime::LoopTaskRecord::new_for_test("loop-1", "ship");
        task.session_id = Some("session-1".to_string());
        journal
            .append(crate::loop_runtime::LoopEventEnvelope::task_created(
                task, None, None,
            ))
            .unwrap();

        let mismatched = record_loop_review_decision_for_a2a(
            &journal,
            &projection,
            "session-2",
            Some("loop-1"),
            AgentReviewDecision::Approve,
            "",
            42,
        )
        .expect("bridge result");
        let missing = record_loop_review_decision_for_a2a(
            &journal,
            &projection,
            "session-1",
            None,
            AgentReviewDecision::Approve,
            "",
            43,
        )
        .expect("bridge result");

        assert!(!mismatched);
        assert!(!missing);
        let replayed = projection.load_or_rebuild(&journal).unwrap();
        let task = replayed.find("loop-1").expect("loop task");
        assert!(task.evidence.is_empty());
    }
}
