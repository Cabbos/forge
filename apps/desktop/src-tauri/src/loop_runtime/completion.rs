use crate::loop_runtime::gates::{HumanGateDecision, HumanGateDecisionKind};
use crate::loop_runtime::types::{
    CompletionFactBucket, CompletionFactStatus, EvidenceRecord, LoopCompletionEligibilityFacts,
    LoopCompletionResult, LoopCompletionStatus, LoopReviewStatus, LoopTaskRecord, LoopTaskStatus,
};

pub fn evaluate_completion(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
) -> LoopCompletionResult {
    let contract = &task.completion_contract;

    let runtime_block_reasons = runtime_block_reasons(task);
    if !runtime_block_reasons.is_empty() {
        return result_with_review_facts(
            LoopCompletionStatus::Blocked,
            runtime_block_reasons.clone(),
            completion_facts(task, evidence, &runtime_block_reasons),
        );
    }

    if contract.stop_on_budget_exceeded && evidence.iter().any(is_budget_exceeded) {
        let reasons = vec!["budget_exceeded".to_string()];
        return result_with_review_facts(
            LoopCompletionStatus::FailedBudget,
            reasons.clone(),
            completion_facts(task, evidence, &reasons),
        );
    }

    let missing_checks = contract
        .required_checks
        .iter()
        .filter(|check| !has_successful_check(evidence, check))
        .map(|check| format!("missing_required_check:{check}"))
        .collect::<Vec<_>>();
    if !missing_checks.is_empty() {
        return result_with_review_facts(
            LoopCompletionStatus::Blocked,
            missing_checks.clone(),
            completion_facts(task, evidence, &missing_checks),
        );
    }

    if let Some(max_risk) = contract.max_gitnexus_risk.as_deref() {
        let Some(max_risk_level) = RiskLevel::parse(max_risk) else {
            let reasons = vec![format!("invalid_max_gitnexus_risk:{max_risk}")];
            return result_with_review_facts(
                LoopCompletionStatus::Blocked,
                reasons.clone(),
                completion_facts(task, evidence, &reasons),
            );
        };
        let Some(actual_risk) = latest_gitnexus_risk(evidence) else {
            let reasons = vec!["missing_gitnexus_risk".to_string()];
            return result_with_review_facts(
                LoopCompletionStatus::Blocked,
                reasons.clone(),
                completion_facts(task, evidence, &reasons),
            );
        };
        let Some(actual_risk_level) = RiskLevel::parse(actual_risk) else {
            let reasons = vec![format!("invalid_gitnexus_risk:{actual_risk}")];
            return result_with_review_facts(
                LoopCompletionStatus::FailedRisk,
                reasons.clone(),
                completion_facts(task, evidence, &reasons),
            );
        };
        if actual_risk_level > max_risk_level {
            let reasons = vec![format!("gitnexus_risk_exceeded:{actual_risk}>{max_risk}")];
            return result_with_review_facts(
                LoopCompletionStatus::FailedRisk,
                reasons.clone(),
                completion_facts(task, evidence, &reasons),
            );
        }
    }

    let mut blockers = Vec::new();
    if contract.require_docs && !has_docs_evidence(evidence) {
        blockers.push("missing_docs");
    }
    if !blockers.is_empty() {
        let blockers = blockers.into_iter().map(str::to_string).collect::<Vec<_>>();
        return result_with_review_facts(
            LoopCompletionStatus::Blocked,
            blockers.clone(),
            completion_facts(task, evidence, &blockers),
        );
    }

    let facts = completion_facts(task, evidence, &[]);
    if matches!(facts.review_status, LoopReviewStatus::Rejected) {
        return result_with_review_facts(
            LoopCompletionStatus::WaitingForReview,
            facts.commit_blockers.clone(),
            facts,
        );
    }

    let review_wait_reasons = review_wait_reasons(task);
    if !review_wait_reasons.is_empty() {
        return result_with_review_facts(
            LoopCompletionStatus::WaitingForReview,
            review_wait_reasons,
            facts,
        );
    }

    if contract.require_review_decision && !has_approved_review_decision(evidence) {
        return result_with_review_facts(
            LoopCompletionStatus::WaitingForReview,
            vec!["missing_review_decision".to_string()],
            facts,
        );
    }

    if contract.require_commit {
        if let Some(commit_blocker) = commit_evidence_blocker(evidence) {
            return result_with_review_facts(
                LoopCompletionStatus::Blocked,
                vec![commit_blocker],
                facts,
            );
        }
    }

    result_with_review_facts(LoopCompletionStatus::Complete, Vec::new(), facts)
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

fn result_with_review_facts(
    status: LoopCompletionStatus,
    reasons: Vec<String>,
    facts: CompletionFacts,
) -> LoopCompletionResult {
    LoopCompletionResult {
        status,
        reasons,
        review_status: facts.review_status,
        commit_eligible: facts.commit_eligible,
        commit_blockers: facts.commit_blockers,
        human_gate_id: facts.human_gate_id,
        last_review_decision: facts.last_review_decision,
        eligibility_facts: facts.eligibility_facts,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompletionFacts {
    review_status: LoopReviewStatus,
    commit_eligible: bool,
    commit_blockers: Vec<String>,
    human_gate_id: Option<String>,
    last_review_decision: Option<HumanGateDecision>,
    eligibility_facts: LoopCompletionEligibilityFacts,
}

fn completion_facts(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    contract_blockers: &[String],
) -> CompletionFacts {
    let latest_review = latest_review_decision(evidence);
    let human_gate_id = latest_review
        .as_ref()
        .map(|(gate_id, _)| (*gate_id).to_string())
        .or_else(|| task.open_gates.first().map(|gate| gate.gate_id.clone()));
    let last_review_decision = latest_review
        .as_ref()
        .map(|(_, decision)| (*decision).clone());
    let mut commit_blockers = contract_blockers.to_vec();
    for blocker in open_gate_blockers(task) {
        push_unique(&mut commit_blockers, blocker);
    }
    if task.completion_contract.require_commit {
        if let Some(blocker) = commit_evidence_blocker(evidence) {
            push_unique(&mut commit_blockers, blocker);
        }
    }
    let review_required =
        task.completion_contract.require_review_decision || !task.open_gates.is_empty();

    let review_status = match latest_review {
        Some((_, decision)) if decision.kind == HumanGateDecisionKind::Approved => {
            LoopReviewStatus::Approved
        }
        Some((_, decision)) => {
            push_unique(&mut commit_blockers, review_blocker(decision));
            LoopReviewStatus::Rejected
        }
        None if !review_required => LoopReviewStatus::NotRequired,
        None if contract_blockers.is_empty() => {
            push_unique(&mut commit_blockers, "missing_human_review".to_string());
            LoopReviewStatus::ReadyForReview
        }
        None => {
            push_unique(&mut commit_blockers, "missing_human_review".to_string());
            LoopReviewStatus::Blocked
        }
    };

    let commit_eligible = commit_blockers.is_empty()
        && matches!(
            review_status,
            LoopReviewStatus::Approved | LoopReviewStatus::NotRequired
        );
    let eligibility_facts = completion_eligibility_facts(
        task,
        evidence,
        contract_blockers,
        &commit_blockers,
        review_status,
    );

    CompletionFacts {
        review_status,
        commit_eligible,
        commit_blockers,
        human_gate_id,
        last_review_decision,
        eligibility_facts,
    }
}

fn completion_eligibility_facts(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    contract_blockers: &[String],
    commit_blockers: &[String],
    review_status: LoopReviewStatus,
) -> LoopCompletionEligibilityFacts {
    LoopCompletionEligibilityFacts {
        verification: verification_fact(task, evidence, contract_blockers),
        changed_file_scope: CompletionFactBucket::new(
            CompletionFactStatus::Unknown,
            "changed_file_scope_not_connected_to_completion_contract",
        ),
        permission: CompletionFactBucket::new(
            CompletionFactStatus::Unknown,
            "permission_evidence_not_connected_to_completion_contract",
        ),
        review: review_fact(review_status, commit_blockers),
        docs: docs_fact(task, evidence, contract_blockers),
        eval: CompletionFactBucket::new(
            CompletionFactStatus::Unknown,
            "eval_evidence_not_connected_to_completion_contract",
        ),
        residual_risk: residual_risk_fact(task, evidence, contract_blockers),
        commit: commit_fact(task, evidence, commit_blockers),
    }
}

fn verification_fact(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    contract_blockers: &[String],
) -> CompletionFactBucket {
    if task.completion_contract.required_checks.is_empty() {
        return CompletionFactBucket::new(CompletionFactStatus::NotRequired, "no_required_checks");
    }
    let blockers = contract_blockers
        .iter()
        .filter(|reason| reason.starts_with("missing_required_check:"))
        .cloned()
        .collect::<Vec<_>>();
    if !blockers.is_empty() {
        return CompletionFactBucket::new(CompletionFactStatus::Missing, "required_checks_missing")
            .with_blockers(blockers);
    }
    let evidence_ids = evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRecord::Command {
                evidence_id,
                check_name,
                success: true,
                ..
            } if task
                .completion_contract
                .required_checks
                .iter()
                .any(|required| required == check_name) =>
            {
                Some(evidence_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    CompletionFactBucket::new(CompletionFactStatus::Satisfied, "required_checks_satisfied")
        .with_evidence_ids(evidence_ids)
}

fn docs_fact(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    contract_blockers: &[String],
) -> CompletionFactBucket {
    if !task.completion_contract.require_docs {
        return CompletionFactBucket::new(CompletionFactStatus::NotRequired, "docs_not_required");
    }
    let blockers = contract_blockers
        .iter()
        .filter(|reason| reason.as_str() == "missing_docs")
        .cloned()
        .collect::<Vec<_>>();
    if !blockers.is_empty() {
        return CompletionFactBucket::new(CompletionFactStatus::Missing, "docs_missing")
            .with_blockers(blockers);
    }
    let evidence_ids = evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRecord::Docs { evidence_id, paths } if !paths.is_empty() => {
                Some(evidence_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    CompletionFactBucket::new(CompletionFactStatus::Satisfied, "docs_evidence_satisfied")
        .with_evidence_ids(evidence_ids)
}

fn residual_risk_fact(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    contract_blockers: &[String],
) -> CompletionFactBucket {
    if task.completion_contract.max_gitnexus_risk.is_none() {
        return CompletionFactBucket::new(
            CompletionFactStatus::NotRequired,
            "residual_risk_not_required",
        );
    }
    let blockers = contract_blockers
        .iter()
        .filter(|reason| {
            reason.as_str() == "missing_gitnexus_risk"
                || reason.starts_with("invalid_max_gitnexus_risk:")
                || reason.starts_with("invalid_gitnexus_risk:")
                || reason.starts_with("gitnexus_risk_exceeded:")
        })
        .cloned()
        .collect::<Vec<_>>();
    if !blockers.is_empty() {
        let status = if blockers
            .iter()
            .any(|blocker| blocker.as_str() == "missing_gitnexus_risk")
        {
            CompletionFactStatus::Missing
        } else {
            CompletionFactStatus::Blocked
        };
        return CompletionFactBucket::new(status, "residual_risk_blocked").with_blockers(blockers);
    }
    let evidence_ids = evidence
        .iter()
        .rev()
        .find_map(|evidence| match evidence {
            EvidenceRecord::GitNexus { evidence_id, .. } => Some(vec![evidence_id.clone()]),
            _ => None,
        })
        .unwrap_or_default();
    if evidence_ids.is_empty() {
        CompletionFactBucket::new(CompletionFactStatus::Missing, "residual_risk_missing")
            .with_blockers(vec!["missing_gitnexus_risk".to_string()])
    } else {
        CompletionFactBucket::new(CompletionFactStatus::Satisfied, "residual_risk_satisfied")
            .with_evidence_ids(evidence_ids)
    }
}

fn review_fact(
    review_status: LoopReviewStatus,
    commit_blockers: &[String],
) -> CompletionFactBucket {
    match review_status {
        LoopReviewStatus::NotRequired => {
            CompletionFactBucket::new(CompletionFactStatus::NotRequired, "review_not_required")
        }
        LoopReviewStatus::Approved => {
            CompletionFactBucket::new(CompletionFactStatus::Satisfied, "review_approved")
        }
        LoopReviewStatus::ReadyForReview => {
            CompletionFactBucket::new(CompletionFactStatus::Missing, "review_decision_missing")
                .with_blockers(review_blockers(commit_blockers))
        }
        LoopReviewStatus::Rejected => {
            CompletionFactBucket::new(CompletionFactStatus::Blocked, "review_rejected")
                .with_blockers(review_blockers(commit_blockers))
        }
        LoopReviewStatus::Blocked => {
            CompletionFactBucket::new(CompletionFactStatus::Blocked, "review_blocked")
                .with_blockers(review_blockers(commit_blockers))
        }
    }
}

fn commit_fact(
    task: &LoopTaskRecord,
    evidence: &[EvidenceRecord],
    commit_blockers: &[String],
) -> CompletionFactBucket {
    if !task.completion_contract.require_commit {
        return CompletionFactBucket::new(CompletionFactStatus::NotRequired, "commit_not_required");
    }
    let blockers = commit_blockers
        .iter()
        .filter(|blocker| {
            blocker.as_str() == "missing_commit"
                || blocker.as_str() == "commit_missing_human_gate"
                || blocker.starts_with("commit_without_approved_human_gate:")
        })
        .cloned()
        .collect::<Vec<_>>();
    if !blockers.is_empty() {
        let status = if blockers
            .iter()
            .any(|blocker| blocker.as_str() == "missing_commit")
        {
            CompletionFactStatus::Missing
        } else {
            CompletionFactStatus::Blocked
        };
        return CompletionFactBucket::new(status, "commit_blocked").with_blockers(blockers);
    }
    let evidence_ids = evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRecord::Commit { evidence_id, .. } => Some(evidence_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    CompletionFactBucket::new(CompletionFactStatus::Satisfied, "commit_evidence_satisfied")
        .with_evidence_ids(evidence_ids)
}

fn review_blockers(commit_blockers: &[String]) -> Vec<String> {
    commit_blockers
        .iter()
        .filter(|blocker| {
            blocker.as_str() == "missing_human_review"
                || blocker.starts_with("review_rejected:")
                || blocker.starts_with("review_canceled:")
                || blocker.as_str() == "review_rejected"
                || blocker.as_str() == "review_canceled"
                || blocker.starts_with("open_human_gate:")
        })
        .cloned()
        .collect()
}

fn latest_review_decision(evidence: &[EvidenceRecord]) -> Option<(&str, &HumanGateDecision)> {
    evidence.iter().rev().find_map(|evidence| match evidence {
        EvidenceRecord::Review {
            gate_id, decision, ..
        } => Some((gate_id.as_str(), decision)),
        _ => None,
    })
}

fn has_approved_review_decision(evidence: &[EvidenceRecord]) -> bool {
    evidence.iter().any(|evidence| {
        matches!(
            evidence,
            EvidenceRecord::Review { decision, .. }
                if decision.kind == HumanGateDecisionKind::Approved
        )
    })
}

fn review_blocker(decision: &HumanGateDecision) -> String {
    let prefix = match decision.kind {
        HumanGateDecisionKind::Approved => "review_approved",
        HumanGateDecisionKind::Denied => "review_rejected",
        HumanGateDecisionKind::Canceled => "review_canceled",
    };
    match decision
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    {
        Some(reason) => format!("{prefix}:{reason}"),
        None => prefix.to_string(),
    }
}

fn open_gate_blockers(task: &LoopTaskRecord) -> Vec<String> {
    task.open_gates
        .iter()
        .map(|gate| format!("open_human_gate:{}", gate.gate_id))
        .collect()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn commit_evidence_blocker(evidence: &[EvidenceRecord]) -> Option<String> {
    let approved_gate_ids = evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRecord::Review {
                gate_id, decision, ..
            } if decision.kind == HumanGateDecisionKind::Approved => Some(gate_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut saw_commit = false;
    let mut first_blocker = None;
    for evidence in evidence {
        let EvidenceRecord::Commit {
            commit_sha,
            human_gate_id,
            ..
        } = evidence
        else {
            continue;
        };
        if commit_sha.trim().is_empty() {
            continue;
        }
        saw_commit = true;
        let Some(human_gate_id) = human_gate_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            first_blocker.get_or_insert_with(|| "commit_missing_human_gate".to_string());
            continue;
        };
        if approved_gate_ids.contains(&human_gate_id) {
            return None;
        }
        first_blocker
            .get_or_insert_with(|| format!("commit_without_approved_human_gate:{human_gate_id}"));
    }

    if saw_commit {
        first_blocker
    } else {
        Some("missing_commit".to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        evaluate_completion, EvidenceRecord, HumanGateDecision, HumanGateDecisionKind,
        HumanGateRecord, HumanGateType, LoopCompletionContract, LoopCompletionStatus,
        LoopReviewStatus, LoopTaskOutcome, LoopTaskRecord, LoopTaskStatus,
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
                EvidenceRecord::Review {
                    evidence_id: "evidence-review".to_string(),
                    gate_id: "gate-1".to_string(),
                    decision: HumanGateDecision::approved(Some("reviewer".to_string()), None),
                },
                EvidenceRecord::Commit {
                    evidence_id: "evidence-commit".to_string(),
                    commit_sha: "abc1234".to_string(),
                    summary: "test commit".to_string(),
                    human_gate_id: Some("gate-1".to_string()),
                },
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
    fn satisfied_build_test_docs_contract_marks_ready_for_review_without_commit_eligibility() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string(), "test:desktop".to_string()],
                max_gitnexus_risk: None,
                require_docs: true,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::command_for_test("build:desktop", true),
                EvidenceRecord::command_for_test("test:desktop", true),
                EvidenceRecord::docs_for_test(vec!["README.md"]),
            ],
        );

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.review_status, LoopReviewStatus::ReadyForReview);
        assert!(!result.commit_eligible);
        assert_eq!(result.commit_blockers, vec!["missing_human_review"]);
        assert_eq!(result.human_gate_id, None);
        assert_eq!(result.last_review_decision, None);
    }

    #[test]
    fn completion_result_includes_v2_eligibility_fact_buckets() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: Some("medium".to_string()),
                require_docs: true,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::command_for_test("build:desktop", true),
                EvidenceRecord::gitnexus_for_test("low"),
                EvidenceRecord::docs_for_test(vec!["README.md"]),
            ],
        );

        assert_eq!(
            result.eligibility_facts.verification.status,
            crate::loop_runtime::CompletionFactStatus::Satisfied
        );
        assert_eq!(
            result.eligibility_facts.verification.evidence_ids,
            vec!["evidence-command-build:desktop"]
        );
        assert_eq!(
            result.eligibility_facts.docs.status,
            crate::loop_runtime::CompletionFactStatus::Satisfied
        );
        assert_eq!(
            result.eligibility_facts.residual_risk.status,
            crate::loop_runtime::CompletionFactStatus::Satisfied
        );
        assert_eq!(
            result.eligibility_facts.review.status,
            crate::loop_runtime::CompletionFactStatus::Missing
        );
        assert_eq!(
            result.eligibility_facts.review.blockers,
            vec!["missing_human_review"]
        );
        assert_eq!(
            result.eligibility_facts.changed_file_scope.status,
            crate::loop_runtime::CompletionFactStatus::Unknown
        );
        assert_eq!(
            result.eligibility_facts.permission.status,
            crate::loop_runtime::CompletionFactStatus::Unknown
        );
        assert_eq!(
            result.eligibility_facts.eval.status,
            crate::loop_runtime::CompletionFactStatus::Unknown
        );
    }

    #[test]
    fn satisfied_contract_without_review_requirement_is_not_human_review_blocked() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: None,
                require_docs: true,
                require_commit: false,
                require_review_decision: false,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::command_for_test("build:desktop", true),
                EvidenceRecord::docs_for_test(vec!["README.md"]),
            ],
        );

        assert_eq!(result.status, LoopCompletionStatus::Complete);
        assert_eq!(result.review_status, LoopReviewStatus::NotRequired);
        assert!(result.commit_eligible);
        assert!(result.commit_blockers.is_empty());
        assert_eq!(result.human_gate_id, None);
        assert_eq!(result.last_review_decision, None);
    }

    #[test]
    fn rejected_review_keeps_commit_ineligible_and_records_reason() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: vec!["build:desktop".to_string()],
                max_gitnexus_risk: None,
                require_docs: true,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::command_for_test("build:desktop", true),
                EvidenceRecord::docs_for_test(vec!["README.md"]),
                EvidenceRecord::Review {
                    evidence_id: "evidence-review".to_string(),
                    gate_id: "gate-1".to_string(),
                    decision: HumanGateDecision::denied(
                        Some("reviewer".to_string()),
                        Some("needs tests".to_string()),
                    ),
                },
            ],
        );

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.review_status, LoopReviewStatus::Rejected);
        assert!(!result.commit_eligible);
        assert_eq!(result.commit_blockers, vec!["review_rejected:needs tests"]);
        assert_eq!(result.human_gate_id.as_deref(), Some("gate-1"));
        let decision = result
            .last_review_decision
            .as_ref()
            .expect("review decision");
        assert_eq!(decision.kind, HumanGateDecisionKind::Denied);
        assert_eq!(decision.reason.as_deref(), Some("needs tests"));
    }

    #[test]
    fn commit_evidence_requires_closed_human_gate_reference() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: true,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });
        let approved_review = EvidenceRecord::Review {
            evidence_id: "evidence-review".to_string(),
            gate_id: "gate-1".to_string(),
            decision: HumanGateDecision::approved(Some("reviewer".to_string()), None),
        };
        let commit_without_gate = EvidenceRecord::Commit {
            evidence_id: "evidence-commit".to_string(),
            commit_sha: "abc1234".to_string(),
            summary: "test commit".to_string(),
            human_gate_id: None,
        };

        let result = evaluate_completion(&task, &[approved_review.clone(), commit_without_gate]);

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["commit_missing_human_gate"]);
        assert_eq!(result.commit_blockers, vec!["commit_missing_human_gate"]);
        assert!(!result.commit_eligible);

        let commit_with_gate = EvidenceRecord::Commit {
            evidence_id: "evidence-commit-gated".to_string(),
            commit_sha: "def5678".to_string(),
            summary: "human commit".to_string(),
            human_gate_id: Some("gate-1".to_string()),
        };

        let result = evaluate_completion(&task, &[approved_review, commit_with_gate]);

        assert_eq!(result.status, LoopCompletionStatus::Complete);
        assert!(result.reasons.is_empty());
        assert!(result.commit_eligible);
        assert_eq!(result.human_gate_id.as_deref(), Some("gate-1"));
    }

    #[test]
    fn approved_review_with_open_gate_is_not_commit_eligible_and_records_gate_blocker() {
        let mut task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: false,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });
        task.open_gates.push(HumanGateRecord::new(
            "gate-2".to_string(),
            HumanGateType::PolicyOverride,
            "Approve remaining work".to_string(),
        ));

        let result = evaluate_completion(
            &task,
            &[EvidenceRecord::Review {
                evidence_id: "evidence-review".to_string(),
                gate_id: "gate-1".to_string(),
                decision: HumanGateDecision::approved(Some("reviewer".to_string()), None),
            }],
        );

        assert_eq!(result.status, LoopCompletionStatus::WaitingForReview);
        assert_eq!(result.reasons, vec!["open_human_gate:gate-2"]);
        assert_eq!(result.review_status, LoopReviewStatus::Approved);
        assert!(!result.commit_eligible);
        assert_eq!(result.commit_blockers, vec!["open_human_gate:gate-2"]);
    }

    #[test]
    fn commit_evidence_wrong_human_gate_is_structured_commit_blocker() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: true,
                require_review_decision: true,
                stop_on_budget_exceeded: true,
            });

        let result = evaluate_completion(
            &task,
            &[
                EvidenceRecord::Review {
                    evidence_id: "evidence-review".to_string(),
                    gate_id: "gate-1".to_string(),
                    decision: HumanGateDecision::approved(Some("reviewer".to_string()), None),
                },
                EvidenceRecord::Commit {
                    evidence_id: "evidence-commit".to_string(),
                    commit_sha: "abc1234".to_string(),
                    summary: "human commit".to_string(),
                    human_gate_id: Some("gate-2".to_string()),
                },
            ],
        );

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(
            result.reasons,
            vec!["commit_without_approved_human_gate:gate-2"]
        );
        assert_eq!(
            result.commit_blockers,
            vec!["commit_without_approved_human_gate:gate-2"]
        );
        assert!(!result.commit_eligible);
    }

    #[test]
    fn missing_commit_is_structured_commit_blocker_when_required() {
        let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
            .with_completion_contract(LoopCompletionContract {
                required_checks: Vec::new(),
                max_gitnexus_risk: None,
                require_docs: false,
                require_commit: true,
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

        assert_eq!(result.status, LoopCompletionStatus::Blocked);
        assert_eq!(result.reasons, vec!["missing_commit"]);
        assert_eq!(result.commit_blockers, vec!["missing_commit"]);
        assert!(!result.commit_eligible);
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
        assert_eq!(result.reasons, vec!["missing_docs"]);
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

    #[test]
    fn runtime_sources_do_not_shell_out_to_git_commit_merge_or_push() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let runtime_files = [
            "src/loop_runtime/completion.rs",
            "src/loop_runtime/gates.rs",
            "src/loop_runtime/policy.rs",
            "src/loop_runtime/projection.rs",
            "src/loop_runtime/types.rs",
            "src/gateway/protocol.rs",
            "src/gateway/server/mod.rs",
            "src/gateway/server/sessions.rs",
            "src/gateway/server/triggers.rs",
            "src/gateway/server/session_inputs.rs",
            "src/gateway/server/loop_tasks.rs",
            "src/gateway/server/ownership.rs",
            "src/gateway/server/status.rs",
            "src/ipc/a2a_handlers.rs",
            "src/agent/a2a/review_gate.rs",
        ];
        let forbidden = [
            "git commit",
            "git merge",
            "git push",
            ".arg(\"commit\")",
            ".arg(\"merge\")",
            ".arg(\"push\")",
            ".args([\"commit\"",
            ".args([\"merge\"",
            ".args([\"push\"",
        ];

        for relative in runtime_files {
            let raw = std::fs::read_to_string(manifest_dir.join(relative))
                .unwrap_or_else(|error| panic!("read {relative}: {error}"));
            let production_source = raw.split("#[cfg(test)]").next().unwrap_or(&raw);
            for needle in forbidden {
                assert!(
                    !production_source.contains(needle),
                    "runtime production source {relative} must not shell out to {needle}"
                );
            }
        }
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
