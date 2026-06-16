use crate::loop_runtime::types::{
    EvidenceRecord, LoopCompletionResult, LoopCompletionStatus, LoopTaskRecord, LoopTaskStatus,
};

pub fn evaluate_completion(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
) -> LoopCompletionResult {
    let contract = &task.completion_contract;

    let runtime_block_reasons = runtime_block_reasons(task);
    if !runtime_block_reasons.is_empty() {
        return LoopCompletionResult {
            status: LoopCompletionStatus::Blocked,
            reasons: runtime_block_reasons,
        };
    }

    let review_wait_reasons = review_wait_reasons(task);
    if !review_wait_reasons.is_empty() {
        return LoopCompletionResult {
            status: LoopCompletionStatus::WaitingForReview,
            reasons: review_wait_reasons,
        };
    }

    if contract.stop_on_budget_exceeded && evidence.iter().any(is_budget_exceeded) {
        return result(LoopCompletionStatus::FailedBudget, vec!["budget_exceeded"]);
    }

    let missing_checks = contract
        .required_checks
        .iter()
        .filter(|check| !has_successful_check(evidence, check))
        .map(|check| format!("missing_required_check:{check}"))
        .collect::<Vec<_>>();
    if !missing_checks.is_empty() {
        return LoopCompletionResult {
            status: LoopCompletionStatus::Blocked,
            reasons: missing_checks,
        };
    }

    if let Some(max_risk) = contract.max_gitnexus_risk.as_deref() {
        let Some(max_risk_level) = RiskLevel::parse(max_risk) else {
            return result(
                LoopCompletionStatus::Blocked,
                vec![format!("invalid_max_gitnexus_risk:{max_risk}")],
            );
        };
        let Some(actual_risk) = latest_gitnexus_risk(evidence) else {
            return result(LoopCompletionStatus::Blocked, vec!["missing_gitnexus_risk"]);
        };
        let Some(actual_risk_level) = RiskLevel::parse(actual_risk) else {
            return result(
                LoopCompletionStatus::FailedRisk,
                vec![format!("invalid_gitnexus_risk:{actual_risk}")],
            );
        };
        if actual_risk_level > max_risk_level {
            return result(
                LoopCompletionStatus::FailedRisk,
                vec![format!("gitnexus_risk_exceeded:{actual_risk}>{max_risk}")],
            );
        }
    }

    let mut blockers = Vec::new();
    if contract.require_docs && !has_docs_evidence(evidence) {
        blockers.push("missing_docs");
    }
    if contract.require_commit && !has_commit_evidence(evidence) {
        blockers.push("missing_commit");
    }
    if !blockers.is_empty() {
        return result(LoopCompletionStatus::Blocked, blockers);
    }

    if contract.require_review_decision && !has_review_decision(evidence) {
        return result(
            LoopCompletionStatus::WaitingForReview,
            vec!["missing_review_decision"],
        );
    }

    LoopCompletionResult {
        status: LoopCompletionStatus::Complete,
        reasons: Vec::new(),
    }
}

fn runtime_block_reasons(task: &LoopTaskRecord) -> Vec<String> {
    match task.status {
        LoopTaskStatus::WaitingForInput => vec!["task_waiting_for_input".to_string()],
        LoopTaskStatus::Interrupted => vec!["task_interrupted".to_string()],
        _ => Vec::new(),
    }
}

fn review_wait_reasons(task: &LoopTaskRecord) -> Vec<String> {
    let mut reasons = Vec::new();
    if task.status == LoopTaskStatus::WaitingForReview {
        reasons.push("task_waiting_for_review".to_string());
    }
    reasons.extend(
        task.open_gates
            .iter()
            .map(|gate| format!("open_human_gate:{}", gate.gate_id)),
    );
    reasons
}

fn result(status: LoopCompletionStatus, reasons: Vec<impl Into<String>>) -> LoopCompletionResult {
    LoopCompletionResult {
        status,
        reasons: reasons.into_iter().map(Into::into).collect(),
    }
}

fn is_budget_exceeded(evidence: &EvidenceRecord) -> bool {
    matches!(
        evidence,
        EvidenceRecord::Budget {
            budget_exceeded: true,
            ..
        }
    )
}

fn has_successful_check(evidence: &[EvidenceRecord], required_check: &str) -> bool {
    evidence.iter().any(|evidence| {
        matches!(
            evidence,
            EvidenceRecord::Command {
                check_name,
                success: true,
                ..
            } if check_name == required_check
        )
    })
}

fn latest_gitnexus_risk(evidence: &[EvidenceRecord]) -> Option<&str> {
    evidence.iter().rev().find_map(|evidence| match evidence {
        EvidenceRecord::GitNexus { risk, .. } => Some(risk.as_str()),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    fn parse(risk: &str) -> Option<Self> {
        match risk.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

fn has_docs_evidence(evidence: &[EvidenceRecord]) -> bool {
    evidence.iter().any(|evidence| {
        matches!(
            evidence,
            EvidenceRecord::Docs { paths, .. } if !paths.is_empty()
        )
    })
}

fn has_commit_evidence(evidence: &[EvidenceRecord]) -> bool {
    evidence.iter().any(|evidence| {
        matches!(
            evidence,
            EvidenceRecord::Commit { commit_sha, .. } if !commit_sha.trim().is_empty()
        )
    })
}

fn has_review_decision(evidence: &[EvidenceRecord]) -> bool {
    evidence
        .iter()
        .any(|evidence| matches!(evidence, EvidenceRecord::Review { .. }))
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        evaluate_completion, EvidenceRecord, HumanGateDecision, HumanGateRecord, HumanGateType,
        LoopCompletionContract, LoopCompletionStatus, LoopTaskOutcome, LoopTaskRecord,
        LoopTaskStatus,
    };

    #[test]
    fn completion_waits_for_required_check() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: Some("medium".to_string()),
                require_docs: true,
                require_commit: true,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["missing_required_check:build:desktop"]);
    }

    #[test]
    fn completion_passes_with_required_evidence() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: Some("medium".to_string()),
                require_docs: true,
                require_commit: true,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::command_for_test("build:desktop", true),
                EvidenceRecord::gitnexus_for_test("medium"),
                EvidenceRecord::docs_for_test(vec!["README.md"]),
                EvidenceRecord::commit_for_test("abc1234"),
            ],
        );

        assert_eq!(result.status, LoopCompletionStatus::Complete);
        assert!(result.reasons.is_empty());
    }

    #[test]
    fn completion_waits_for_review_decision() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["missing_review_decision"]);
    }

    #[test]
    fn completion_waits_when_task_status_is_waiting_for_review() {
        let mut task = LoopTaskRecord::new_for_test("task-1", "finish runtime");
        task.status = LoopTaskStatus::WaitingForReview;

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["task_waiting_for_review"]);
    }

    #[test]
    fn completion_blocks_when_task_status_is_waiting_for_input() {
        let mut task = LoopTaskRecord::new_for_test("task-1", "finish runtime");
        task.status = LoopTaskStatus::WaitingForInput;

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["task_waiting_for_input"]);
    }

    #[test]
    fn completion_blocks_when_task_status_is_interrupted() {
        let mut task = LoopTaskRecord::new_for_test("task-1", "finish runtime");
        task.status = LoopTaskStatus::Interrupted;

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["task_interrupted"]);
    }

    #[test]
    fn completion_waits_when_task_has_open_gate_even_with_sufficient_evidence() {
        let (mut task, evidence) = task_with_sufficient_evidence();
        task.open_gates.push(HumanGateRecord::new(
            "gate-1".to_string(),
            HumanGateType::PolicyOverride,
            "Approve runtime edit".to_string(),
        ));

        let result = evaluate_completion(&task, &evidence);

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["open_human_gate:gate-1"]);
    }

    #[test]
    fn completion_waits_after_denied_gate_status_even_with_sufficient_evidence() {
        let (mut task, evidence) = task_with_sufficient_evidence();
        task.status = LoopTaskStatus::WaitingForReview;
        task.outcome = Some(LoopTaskOutcome {
            status: LoopTaskStatus::WaitingForReview,
            message: "human gate gate-1 denied".to_string(),
            completed_at_ms: 2,
        });

        let result = evaluate_completion(&task, &evidence);

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["task_waiting_for_review"]);
    }

    #[test]
    fn completion_waits_after_canceled_gate_status_even_with_sufficient_evidence() {
        let (mut task, evidence) = task_with_sufficient_evidence();
        task.status = LoopTaskStatus::WaitingForReview;
        task.outcome = Some(LoopTaskOutcome {
            status: LoopTaskStatus::WaitingForReview,
            message: "human gate gate-1 canceled".to_string(),
            completed_at_ms: 2,
        });

        let result = evaluate_completion(&task, &evidence);

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["task_waiting_for_review"]);
    }

    #[test]
    fn completion_fails_when_budget_exceeded() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime");

        let result = evaluate_completion(&task, &[EvidenceRecord::budget_for_test(true)]);

        assert_eq!(result.status, LoopCompletionStatus::FailedBudget);
        assert_eq!(result.reasons, vec!["budget_exceeded"]);
    }

    #[test]
    fn completion_fails_when_gitnexus_risk_exceeds_contract() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: Some("medium".to_string()),
                require_docs: false,
                require_commit: false,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(&task, &[EvidenceRecord::gitnexus_for_test("high")]);

        assert_eq!(result.status, LoopCompletionStatus::FailedRisk);
        assert_eq!(result.reasons, vec!["gitnexus_risk_exceeded:high>medium"]);
    }

    #[test]
    fn completion_blocks_malformed_max_gitnexus_risk() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: Some("medum".to_string()),
                require_docs: false,
                require_commit: false,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(&task, &[EvidenceRecord::gitnexus_for_test("critical")]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["invalid_max_gitnexus_risk:medum"]);
    }

    #[test]
    fn completion_blocks_when_required_docs_and_commit_are_missing() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: true,
                require_commit: true,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(&task, &[]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["missing_docs", "missing_commit"]);
    }

    #[test]
    fn completion_accepts_recorded_review_decision() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[EvidenceRecord::Review {
                evidence_id: "evidence-review".to_string(),
                gate_id: "gate-1".to_string(),
                decision: HumanGateDecision::approved(Some("reviewer".to_string()), None),
            }],
        );

        assert_eq!(result.status, LoopCompletionStatus::Complete);
        assert!(result.reasons.is_empty());
    }

    fn task_with_sufficient_evidence() -> (LoopTaskRecord, Vec<EvidenceRecord>) {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: Some("medium".to_string()),
                require_docs: true,
                require_commit: true,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });
        let evidence = vec![
            EvidenceRecord::command_for_test("build:desktop", true),
            EvidenceRecord::gitnexus_for_test("medium"),
            EvidenceRecord::docs_for_test(vec!["README.md"]),
            EvidenceRecord::commit_for_test("abc1234"),
        ];
        (task, evidence)
    }
}
