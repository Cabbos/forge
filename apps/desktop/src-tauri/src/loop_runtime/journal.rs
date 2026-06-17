use crate::loop_runtime::types::{EvidenceRecord, LoopEventEnvelope, LoopRuntimeEvent};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LoopEventJournal {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendResult {
    pub appended: bool,
    pub event: LoopEventEnvelope,
}

impl LoopEventJournal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn persistent_default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::persistent_at(PathBuf::from(home).join(".forge").join("loop-events.jsonl"))
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self::new(path)
    }

    pub fn load_all(&self) -> Result<Vec<LoopEventEnvelope>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        self.load_all_unlocked()
    }

    fn load_all_unlocked(&self) -> Result<Vec<LoopEventEnvelope>, String> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read loop event journal: {error}")),
        };
        raw.lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                Some(
                    serde_json::from_str::<LoopEventEnvelope>(trimmed).map_err(|error| {
                        format!("corrupt loop event journal line {}: {error}", index + 1)
                    }),
                )
            })
            .collect()
    }

    pub fn append(&self, event: LoopEventEnvelope) -> Result<AppendResult, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        let existing = self.load_all_unlocked()?;
        let event = self.prepare_event(event, &existing);
        self.append_prepared(&event)?;
        Ok(AppendResult {
            appended: true,
            event,
        })
    }

    pub fn append_idempotent(&self, event: LoopEventEnvelope) -> Result<AppendResult, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        let existing = self.load_all_unlocked()?;
        if let Some(key) = event.idempotency_key.as_deref() {
            if let Some(found) = existing
                .iter()
                .find(|existing| existing.idempotency_key.as_deref() == Some(key))
            {
                if event_payload_fingerprint(found)? == event_payload_fingerprint(&event)? {
                    return Ok(AppendResult {
                        appended: false,
                        event: found.clone(),
                    });
                }
                return Err(format!("idempotency conflict for key: {key}"));
            }
        }

        let event = self.prepare_event(event, &existing);
        self.append_prepared(&event)?;
        Ok(AppendResult {
            appended: true,
            event,
        })
    }

    pub fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<LoopEventEnvelope>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        Ok(self
            .load_all_unlocked()?
            .into_iter()
            .find(|event| event.idempotency_key.as_deref() == Some(idempotency_key)))
    }

    fn prepare_event(
        &self,
        mut event: LoopEventEnvelope,
        existing: &[LoopEventEnvelope],
    ) -> LoopEventEnvelope {
        let next_sequence = existing
            .iter()
            .filter(|existing| existing.task_id == event.task_id)
            .map(|existing| existing.sequence)
            .max()
            .unwrap_or(0)
            + 1;
        event.sequence = next_sequence;
        event
    }

    fn append_prepared(&self, event: &LoopEventEnvelope) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create loop event journal dir: {error}"))?;
        }
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(|error| format!("open loop event journal: {error}"))?;
        let json = serde_json::to_string(event)
            .map_err(|error| format!("serialize loop event: {error}"))?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| format!("append loop event: {error}"))?;
        Ok(())
    }
}

fn event_payload_fingerprint(event: &LoopEventEnvelope) -> Result<String, String> {
    let payload = match &event.event {
        LoopRuntimeEvent::TaskCreated { task } => serde_json::json!({
            "type": "task_created",
            "goal": task.goal,
            "session_id": task.session_id,
            "profile_id": task.profile_id,
            "workspace_path": task.workspace_path,
            "owner": task.owner,
            "policy": task.policy,
            "budget": task.budget,
            "completion_contract": task.completion_contract,
        }),
        LoopRuntimeEvent::TaskStarted { task_id, lease } => serde_json::json!({
            "type": "task_started",
            "task_id": task_id,
            "lease": lease,
        }),
        LoopRuntimeEvent::TaskWaitingForInput {
            task_id,
            reason,
            waiting_at_ms: _,
        } => serde_json::json!({
            "type": "task_waiting_for_input",
            "task_id": task_id,
            "reason": reason,
        }),
        LoopRuntimeEvent::TaskInterrupted { task_id, reason } => serde_json::json!({
            "type": "task_interrupted",
            "task_id": task_id,
            "reason": reason,
        }),
        LoopRuntimeEvent::TaskCanceled {
            task_id,
            reason,
            canceled_at_ms: _,
        } => serde_json::json!({
            "type": "task_canceled",
            "task_id": task_id,
            "reason": reason,
        }),
        LoopRuntimeEvent::HumanGateRequested { gate } => serde_json::json!({
            "type": "human_gate_requested",
            "task_id": event.task_id,
            "gate_id": gate.gate_id,
            "gate_type": gate.gate_type,
            "prompt": gate.prompt,
        }),
        LoopRuntimeEvent::HumanGateResolved {
            gate_id,
            decision,
            resolved_at_ms: _,
        } => serde_json::json!({
            "type": "human_gate_resolved",
            "task_id": event.task_id,
            "gate_id": gate_id,
            "decision_kind": decision.kind,
            "decided_by": decision.decided_by,
            "reason": decision.reason,
        }),
        LoopRuntimeEvent::EvidenceRecorded { task_id, evidence } => serde_json::json!({
            "type": "evidence_recorded",
            "task_id": task_id,
            "evidence": evidence_fingerprint(evidence),
        }),
        LoopRuntimeEvent::PolicyDecisionRecorded { task_id, decision } => serde_json::json!({
            "type": "policy_decision_recorded",
            "task_id": task_id,
            "decision": {
                "decision_id": decision.decision_id,
                "intent": decision.intent,
                "allowed": decision.allowed,
                "reason": decision.reason,
                "actor": decision.actor,
            },
        }),
        LoopRuntimeEvent::BudgetSnapshotRecorded { task_id, snapshot } => serde_json::json!({
            "type": "budget_snapshot_recorded",
            "task_id": task_id,
            "snapshot": snapshot,
        }),
        LoopRuntimeEvent::SubagentFileIoRecorded {
            task_id,
            child_task_id,
            path,
            operation,
        } => serde_json::json!({
            "type": "subagent_file_io_recorded",
            "task_id": task_id,
            "child_task_id": child_task_id,
            "path": path,
            "operation": operation,
        }),
        LoopRuntimeEvent::UsageLedgerRecorded { task_id, usage } => serde_json::json!({
            "type": "usage_ledger_recorded",
            "task_id": task_id,
            "usage": usage,
        }),
        LoopRuntimeEvent::CompletionEvaluated { task_id, result } => serde_json::json!({
            "type": "completion_evaluated",
            "task_id": task_id,
            "result": result,
        }),
    };
    serde_json::to_string(&payload)
        .map_err(|error| format!("serialize loop event payload: {error}"))
}

fn evidence_fingerprint(evidence: &EvidenceRecord) -> serde_json::Value {
    match evidence {
        EvidenceRecord::Command {
            evidence_id,
            check_name,
            command,
            exit_code,
            success,
            artifact_hash,
        } => serde_json::json!({
            "kind": "command",
            "evidence_id": evidence_id,
            "check_name": check_name,
            "command": command,
            "exit_code": exit_code,
            "success": success,
            "artifact_hash": artifact_hash,
        }),
        EvidenceRecord::GitNexus {
            evidence_id,
            risk,
            changed_symbols,
            affected_processes,
            report_hash,
        } => serde_json::json!({
            "kind": "git_nexus",
            "evidence_id": evidence_id,
            "risk": risk,
            "changed_symbols": changed_symbols,
            "affected_processes": affected_processes,
            "report_hash": report_hash,
        }),
        EvidenceRecord::Commit {
            evidence_id,
            commit_sha,
            summary,
        } => serde_json::json!({
            "kind": "commit",
            "evidence_id": evidence_id,
            "commit_sha": commit_sha,
            "summary": summary,
        }),
        EvidenceRecord::Docs { evidence_id, paths } => serde_json::json!({
            "kind": "docs",
            "evidence_id": evidence_id,
            "paths": paths,
        }),
        EvidenceRecord::Review {
            evidence_id,
            gate_id,
            decision,
        } => serde_json::json!({
            "kind": "review",
            "evidence_id": evidence_id,
            "gate_id": gate_id,
            "decision_kind": decision.kind,
            "decided_by": decision.decided_by,
            "reason": decision.reason,
        }),
        EvidenceRecord::Budget {
            evidence_id,
            budget_exceeded,
        } => serde_json::json!({
            "kind": "budget",
            "evidence_id": evidence_id,
            "budget_exceeded": budget_exceeded,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        EvidenceRecord, HumanGateDecision, HumanGateDecisionKind, LoopActionIntent, LoopActor,
        LoopEventEnvelope, LoopEventJournal, LoopRuntimeEvent, LoopTaskProjection, LoopTaskStatus,
        PolicyDecisionRecord, LOOP_RUNTIME_SCHEMA_VERSION,
    };
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn loop_event_journal_appends_and_replays_created_task() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path.clone());
        let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship Level 3 runtime");

        journal.append(event.clone()).unwrap();

        let loaded = LoopEventJournal::new(path).load_all().unwrap();
        assert_eq!(loaded, vec![event.clone()]);

        let projection = LoopTaskProjection::from_events(&loaded).unwrap();
        assert_eq!(projection.tasks[0].id, "loop-1");
        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Pending);
    }

    #[test]
    fn duplicate_idempotency_key_does_not_append_twice() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
            .with_idempotency_key("create:profile-settings-acceptance");

        let first = journal.append_idempotent(event.clone()).unwrap();
        let second = journal.append_idempotent(event).unwrap();

        assert!(first.appended);
        assert!(!second.appended);
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn journal_assigns_monotonic_sequence_per_task() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
                    .with_idempotency_key("create:loop-1"),
            )
            .unwrap();
        journal
            .append_idempotent(
                LoopEventEnvelope::task_canceled_for_test("loop-1", "done")
                    .with_idempotency_key("cancel:loop-1:done"),
            )
            .unwrap();

        let loaded = journal.load_all().unwrap();
        assert_eq!(loaded[0].sequence, 1);
        assert_eq!(loaded[1].sequence, 2);
    }

    #[test]
    fn conflicting_idempotency_key_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "first")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap();
        let error = journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-2", "second")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_different_create_payload_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "first")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap();
        let error = journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "second")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_semantically_same_gate_resolution_reuses_existing_event() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        let first = human_gate_resolved_event_for_test(
            "loop-1",
            "gate-1",
            HumanGateDecisionKind::Approved,
            10,
        )
        .with_idempotency_key("resolve:gate-1");
        let second = human_gate_resolved_event_for_test(
            "loop-1",
            "gate-1",
            HumanGateDecisionKind::Approved,
            20,
        )
        .with_idempotency_key("resolve:gate-1");

        let first_result = journal.append_idempotent(first).unwrap();
        let second_result = journal.append_idempotent(second).unwrap();

        assert!(first_result.appended);
        assert!(!second_result.appended);
        assert_eq!(second_result.event, first_result.event);
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_different_gate_resolution_decision_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        journal
            .append_idempotent(
                human_gate_resolved_event_for_test(
                    "loop-1",
                    "gate-1",
                    HumanGateDecisionKind::Approved,
                    10,
                )
                .with_idempotency_key("resolve:gate-1"),
            )
            .unwrap();

        let error = journal
            .append_idempotent(
                human_gate_resolved_event_for_test(
                    "loop-1",
                    "gate-1",
                    HumanGateDecisionKind::Denied,
                    20,
                )
                .with_idempotency_key("resolve:gate-1"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_semantically_same_review_evidence_reuses_existing_event() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        let first =
            review_evidence_event_for_test("loop-1", "gate-1", HumanGateDecisionKind::Approved, 10)
                .with_idempotency_key("evidence:review:gate-1");
        let second =
            review_evidence_event_for_test("loop-1", "gate-1", HumanGateDecisionKind::Approved, 20)
                .with_idempotency_key("evidence:review:gate-1");

        let first_result = journal.append_idempotent(first).unwrap();
        let second_result = journal.append_idempotent(second).unwrap();

        assert!(first_result.appended);
        assert!(!second_result.appended);
        assert_eq!(second_result.event, first_result.event);
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_different_review_evidence_decision_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        journal
            .append_idempotent(
                review_evidence_event_for_test(
                    "loop-1",
                    "gate-1",
                    HumanGateDecisionKind::Approved,
                    10,
                )
                .with_idempotency_key("evidence:review:gate-1"),
            )
            .unwrap();

        let error = journal
            .append_idempotent(
                review_evidence_event_for_test(
                    "loop-1",
                    "gate-1",
                    HumanGateDecisionKind::Denied,
                    20,
                )
                .with_idempotency_key("evidence:review:gate-1"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_semantically_same_new_event_payload_reuses_existing_event() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        let first = policy_decision_event_for_test("loop-1", "decision-1", true, "allowed", 10)
            .with_idempotency_key("policy:decision-1");
        let second = policy_decision_event_for_test("loop-1", "decision-1", true, "allowed", 20)
            .with_idempotency_key("policy:decision-1");

        let first_result = journal.append_idempotent(first).unwrap();
        let second_result = journal.append_idempotent(second).unwrap();

        assert!(first_result.appended);
        assert!(!second_result.appended);
        assert_eq!(second_result.event, first_result.event);
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_changed_new_event_payload_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        journal
            .append_idempotent(
                policy_decision_event_for_test("loop-1", "decision-1", true, "allowed", 10)
                    .with_idempotency_key("policy:decision-1"),
            )
            .unwrap();

        let error = journal
            .append_idempotent(
                policy_decision_event_for_test("loop-1", "decision-2", true, "allowed", 20)
                    .with_idempotency_key("policy:decision-1"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn concurrent_duplicate_idempotency_key_appends_once() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = Arc::new(LoopEventJournal::new(path));
        let barrier = Arc::new(Barrier::new(16));
        let mut handles = Vec::new();

        for _ in 0..16 {
            let journal = Arc::clone(&journal);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
                    .with_idempotency_key("create:loop-1");
                barrier.wait();
                journal.append_idempotent(event).unwrap()
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let loaded = journal.load_all().unwrap();
        let sequences = loaded
            .iter()
            .map(|event| event.sequence)
            .collect::<HashSet<_>>();

        assert_eq!(results.iter().filter(|result| result.appended).count(), 1);
        assert_eq!(loaded.len(), 1);
        assert_eq!(sequences.len(), loaded.len());
    }

    #[test]
    fn corrupt_journal_line_reports_line_number() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let valid = serde_json::to_string(&LoopEventEnvelope::task_created_for_test(
            "loop-1",
            "ship runtime",
        ))
        .unwrap();
        std::fs::write(&path, format!("{valid}\n{{not json\n")).unwrap();

        let error = LoopEventJournal::new(path).load_all().unwrap_err();

        assert!(error.contains("line 2"));
    }

    fn human_gate_resolved_event_for_test(
        task_id: &str,
        gate_id: &str,
        kind: HumanGateDecisionKind,
        timestamp: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-{gate_id}-{timestamp}"),
            task_id: task_id.to_string(),
            sequence: 0,
            event: LoopRuntimeEvent::HumanGateResolved {
                gate_id: gate_id.to_string(),
                decision: HumanGateDecision {
                    kind,
                    decided_at_ms: timestamp,
                    decided_by: Some("reviewer".to_string()),
                    reason: Some("reviewed".to_string()),
                },
                resolved_at_ms: timestamp + 1,
            },
            actor: LoopActor::User {
                source: "test".to_string(),
            },
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: timestamp + 2,
        }
    }

    fn review_evidence_event_for_test(
        task_id: &str,
        gate_id: &str,
        kind: HumanGateDecisionKind,
        timestamp: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope::evidence_recorded(
            task_id.to_string(),
            EvidenceRecord::Review {
                evidence_id: format!("evidence-review-{gate_id}"),
                gate_id: gate_id.to_string(),
                decision: HumanGateDecision {
                    kind,
                    decided_at_ms: timestamp,
                    decided_by: Some("reviewer".to_string()),
                    reason: Some("reviewed".to_string()),
                },
            },
            None,
            None,
        )
    }

    fn policy_decision_event_for_test(
        task_id: &str,
        decision_id: &str,
        allowed: bool,
        reason: &str,
        timestamp: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-{decision_id}-{timestamp}"),
            task_id: task_id.to_string(),
            sequence: 0,
            event: LoopRuntimeEvent::PolicyDecisionRecorded {
                task_id: task_id.to_string(),
                decision: PolicyDecisionRecord {
                    decision_id: decision_id.to_string(),
                    intent: LoopActionIntent::Commit {
                        completion_contract_satisfied: false,
                        passing_evidence: false,
                    },
                    allowed,
                    reason: reason.to_string(),
                    actor: LoopActor::Gateway,
                    created_at_ms: timestamp,
                },
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: timestamp,
        }
    }
}
