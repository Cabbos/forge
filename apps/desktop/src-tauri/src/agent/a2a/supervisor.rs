use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::a2a::types::{AgentExecutionMode, AgentRole, AgentTaskId, PatchProposal};
use crate::agent::a2a::worktree::WorktreeWorkerSummary;

pub(crate) fn delegate_result_for_model(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("result")
                .and_then(|result| result.as_str())
                .map(|result| result.to_string())
        })
        .unwrap_or_else(|| raw.to_string())
}

pub(crate) fn assign_delegate_task(
    bus: &mut AgentA2ABus,
    title: &str,
    prompt: &str,
    timestamp_ms: u64,
) -> AgentTaskId {
    bus.assign_task(
        AgentRole::Researcher,
        AgentExecutionMode::ReadOnly,
        title,
        prompt,
        timestamp_ms,
    )
}

pub(crate) fn assign_patch_proposal_task(
    bus: &mut AgentA2ABus,
    title: &str,
    prompt: &str,
    timestamp_ms: u64,
) -> AgentTaskId {
    bus.assign_task(
        AgentRole::Implementer,
        AgentExecutionMode::PatchProposal,
        title,
        prompt,
        timestamp_ms,
    )
}

pub(crate) fn assign_worktree_worker_task(
    bus: &mut AgentA2ABus,
    title: &str,
    prompt: &str,
    timestamp_ms: u64,
) -> AgentTaskId {
    bus.assign_task(
        AgentRole::Implementer,
        AgentExecutionMode::WorktreeWorker,
        title,
        prompt,
        timestamp_ms,
    )
}

pub(crate) fn extract_patch_proposal(raw: &str) -> Option<PatchProposal> {
    // Try to find a JSON block containing patch_proposal.
    // Prefer raw JSON first because wrapped sub-agent results may contain escaped
    // fenced blocks inside their "result" field.
    extract_patch_proposal_from_json_text(raw).or_else(|| {
        let json_text = extract_json_block(raw)?;
        extract_patch_proposal_from_json_text(json_text)
    })
}

fn extract_patch_proposal_from_json_text(json_text: &str) -> Option<PatchProposal> {
    let value = serde_json::from_str::<serde_json::Value>(json_text).ok()?;
    if let Some(proposal) = patch_proposal_from_value(&value) {
        return Some(proposal);
    }

    let result = value.get("result").and_then(|result| result.as_str())?;
    extract_json_block(result)
        .and_then(extract_patch_proposal_from_json_text)
        .or_else(|| extract_patch_proposal_from_json_text(result))
}

fn patch_proposal_from_value(value: &serde_json::Value) -> Option<PatchProposal> {
    value
        .get("patch_proposal")
        .cloned()
        .and_then(|proposal| serde_json::from_value::<PatchProposal>(proposal).ok())
}

fn extract_json_block(text: &str) -> Option<&str> {
    // Find ```json ... ``` or ``` ... ``` block
    if let Some(start) = text.find("```json") {
        let after_marker = &text[start + 7..];
        if let Some(end) = after_marker.find("```") {
            return Some(after_marker[..end].trim());
        }
    }
    if let Some(start) = text.find("```") {
        let after_marker = &text[start + 3..];
        if let Some(end) = after_marker.find("```") {
            let block = after_marker[..end].trim();
            // Only return if it looks like JSON (starts with {)
            if block.starts_with('{') {
                return Some(block);
            }
        }
    }
    None
}

pub(crate) fn record_child_failure(
    bus: &mut AgentA2ABus,
    task_id: &AgentTaskId,
    kind: &str,
    message: &str,
    timestamp_ms: u64,
) {
    bus.fail_task(task_id, kind, message, true, timestamp_ms);
}

/// Extract a human-readable summary string from a worktree-worker JSON result
/// for the parent model's tool_result.
pub(crate) fn worktree_result_for_model(raw: &str) -> String {
    let Ok(summary) =
        serde_json::from_str::<crate::agent::a2a::worktree::WorktreeWorkerSummary>(raw)
    else {
        return delegate_result_for_model(raw);
    };

    let mut lines = Vec::new();
    lines.push(delegate_result_for_model(&summary.result));
    lines.push(format!("Diff available: {}", summary.diff_available));
    if summary.diff_truncated {
        lines.push("Diff was truncated to avoid context overflow.".to_string());
    }
    lines.push(format!("Worktree cleaned up: {}", summary.cleaned_up));

    // Test report with pass/fail status
    if let Some(report) = &summary.test_report {
        if !report.trim().is_empty() {
            lines.push(format!("Test report: {report}"));
        }
    }
    match summary.tests_passed {
        Some(true) => lines.push("Tests: PASSED".to_string()),
        Some(false) => lines.push("Tests: FAILED".to_string()),
        None => {}
    }

    // Review gate: explicit signal that merge is NOT automatic
    if summary.needs_human_review {
        lines.push("HUMAN REVIEW REQUIRED - do not merge automatically.".to_string());
    }
    if !summary.reason_codes.is_empty() {
        lines.push(format!(
            "Review reasons: {}",
            summary.reason_codes.join(", ")
        ));
    }
    if !summary.suggested_action.is_empty() {
        lines.push(format!("Suggested action: {}", summary.suggested_action));
    }

    if !summary.cleaned_up {
        lines.push(format!("Worktree preserved: {}", summary.worktree_path));
    }
    lines.join("\n")
}

/// Extract structured worktree artifacts from the worker JSON result.
pub(crate) fn extract_worktree_artifacts(
    raw: &str,
    task_id: &AgentTaskId,
) -> Vec<crate::agent::a2a::types::AgentArtifact> {
    use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

    let value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut artifacts = Vec::new();
    let now = crate::agent::time::now_ms();

    // DiffSummary artifact
    if let Some(diff) = value.get("diff").and_then(|d| d.as_str()) {
        if !diff.is_empty() {
            artifacts.push(AgentArtifact {
                artifact_id: format!("diff-{}", task_id.as_str()),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::DiffSummary,
                title: "Worktree diff".to_string(),
                content: diff.to_string(),
                created_at_ms: now,
            });
        }
    }

    // TestReport artifact
    if let Some(report) = value.get("test_report").and_then(|r| r.as_str()) {
        if !report.is_empty() {
            artifacts.push(AgentArtifact {
                artifact_id: format!("test-{}", task_id.as_str()),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::TestReport,
                title: "Test report".to_string(),
                content: report.to_string(),
                created_at_ms: now,
            });
        }
    }

    // Worktree metadata artifact (path, cleanup status, review gate)
    if let Ok(summary) =
        serde_json::from_str::<crate::agent::a2a::worktree::WorktreeWorkerSummary>(raw)
    {
        let meta = serde_json::json!({
            "worktree_path": summary.worktree_path,
            "cleaned_up": summary.cleaned_up,
            "diff_available": summary.diff_available,
            "diff_truncated": summary.diff_truncated,
            "tests_passed": summary.tests_passed,
            "needs_human_review": summary.needs_human_review,
            "suggested_action": summary.suggested_action,
            "reason_codes": summary.reason_codes,
        });
        artifacts.push(AgentArtifact {
            artifact_id: format!("meta-{}", task_id.as_str()),
            task_id: task_id.clone(),
            kind: AgentArtifactKind::Evidence,
            title: "Worktree metadata".to_string(),
            content: meta.to_string(),
            created_at_ms: now,
        });
    }

    artifacts
}

/// Arbitration result when multiple worktree workers have completed.
/// This is **always** conservative — never auto-merges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeWorkerArbitration {
    /// Whether the results meet the strict criteria for proposing an apply.
    /// Always `false` in Phase 6 — manual review is mandatory.
    pub can_propose_apply: bool,
    /// Always `true` — human review is required.
    pub needs_human_review: bool,
    /// Human-readable suggested next step.
    pub suggested_action: String,
    /// Whether conflicting signals were detected across workers.
    pub conflict_detected: bool,
    /// Aggregated reason codes from all workers.
    pub aggregated_reasons: Vec<String>,
    pub worker_count: usize,
    pub workers_with_tests_passed: usize,
    pub workers_with_diff_truncated: usize,
    pub workers_with_errors: usize,
}

/// Aggregate results from multiple worktree workers into a conservative
/// arbitration decision. Never auto-merges.
pub(crate) fn arbitrate_worktree_workers(
    summaries: &[WorktreeWorkerSummary],
) -> WorktreeWorkerArbitration {
    let worker_count = summaries.len();

    let workers_with_tests_passed = summaries
        .iter()
        .filter(|s| s.tests_passed == Some(true))
        .count();
    let workers_with_tests_failed = summaries
        .iter()
        .filter(|s| s.tests_passed == Some(false))
        .count();
    let workers_with_diff_truncated = summaries.iter().filter(|s| s.diff_truncated).count();
    let workers_with_no_diff = summaries.iter().filter(|s| !s.diff_available).count();
    let workers_with_errors = summaries
        .iter()
        .filter(|s| {
            s.reason_codes
                .iter()
                .any(|r| r.contains("error") || r.contains("failed"))
        })
        .count();

    let mut reasons = Vec::new();

    if workers_with_tests_failed > 0 {
        reasons.push(format!(
            "{} worker(s) had failing tests",
            workers_with_tests_failed
        ));
    }
    if workers_with_diff_truncated > 0 {
        reasons.push(format!(
            "{} worker(s) had truncated diffs",
            workers_with_diff_truncated
        ));
    }
    if workers_with_errors > 0 {
        reasons.push(format!("{} worker(s) reported errors", workers_with_errors));
    }
    if workers_with_no_diff > 0 {
        reasons.push(format!(
            "{} worker(s) produced no diff",
            workers_with_no_diff
        ));
    }

    // Detect conflicting outcomes across workers.
    let mut conflict_detected = false;
    if worker_count > 1 {
        let distinct_pass_outcomes: std::collections::HashSet<_> =
            summaries.iter().map(|s| s.tests_passed).collect();
        if distinct_pass_outcomes.len() > 1 {
            conflict_detected = true;
            reasons.push("Workers report inconsistent test results".to_string());
        }
        let distinct_truncated: std::collections::HashSet<_> =
            summaries.iter().map(|s| s.diff_truncated).collect();
        if distinct_truncated.len() > 1 && workers_with_diff_truncated > 0 {
            conflict_detected = true;
            reasons.push("Workers have inconsistent diff coverage".to_string());
        }
    }

    let suggested_action = if conflict_detected {
        "HUMAN REVIEW REQUIRED — multiple workers produced conflicting results. \
         Do not merge automatically."
            .to_string()
    } else if !reasons.is_empty() {
        format!(
            "HUMAN REVIEW REQUIRED — {}. Do not merge automatically.",
            reasons.join("; ")
        )
    } else {
        "HUMAN REVIEW REQUIRED — review all worker outputs before merging. \
         Do not merge automatically."
            .to_string()
    };

    WorktreeWorkerArbitration {
        can_propose_apply: false, // Phase 6: never auto-apply
        needs_human_review: true,
        suggested_action,
        conflict_detected,
        aggregated_reasons: reasons,
        worker_count,
        workers_with_tests_passed,
        workers_with_diff_truncated,
        workers_with_errors,
    }
}

/// Apply-proposal contract for a single WorktreeWorker result.
///
/// This function defines the strict criteria that must ALL be met before
/// a worktree result can even be *proposed* for apply. In Phase 6 it
/// always returns `can_propose: false` because automatic merge is not
/// implemented — human confirmation is mandatory.
///
/// **Phase 7 roadmap (not yet implemented):**
/// - User confirmation gate (frontend → backend RPC)
/// - Three-way merge or patch application with rollback
/// - Pre-merge workspace snapshot for recovery
/// - Merge conflict detection and resolution strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyProposal {
    pub can_propose: bool,
    pub user_confirmation_required: bool,
    pub blockers: Vec<String>,
}

pub(crate) fn can_propose_apply(summary: &WorktreeWorkerSummary) -> ApplyProposal {
    let mut blockers = Vec::new();

    // Criterion 1: tests must have passed.
    if summary.tests_passed != Some(true) {
        blockers.push("tests did not pass".to_string());
    }

    // Criterion 2: diff must be available and not truncated.
    if !summary.diff_available {
        blockers.push("no diff available".to_string());
    }
    if summary.diff_truncated {
        blockers.push("diff was truncated".to_string());
    }

    // Criterion 3: no high-risk reason codes.
    let high_risk_reasons = [
        "tests failed",
        "diff was truncated",
        "sub-agent reported an error",
        "no diff was produced",
        "test report could not be parsed",
    ];
    for reason in &summary.reason_codes {
        if high_risk_reasons.iter().any(|hr| reason.contains(hr)) {
            blockers.push(format!("high-risk reason: {reason}"));
        }
    }

    // Criterion 4: worktree must still exist (not cleaned up).
    if summary.cleaned_up {
        blockers.push("worktree was already cleaned up".to_string());
    }

    // Phase 6: always require human confirmation, even if all criteria pass.
    let user_confirmation_required = true;

    // In Phase 6 we never allow propose regardless of criteria.
    // When Phase 7 implements the confirmation gate, this can become:
    //   can_propose = blockers.is_empty() && !user_confirmation_required;
    let can_propose = false;

    ApplyProposal {
        can_propose,
        user_confirmation_required,
        blockers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::bus::AgentA2ABus;

    #[test]
    fn delegate_result_for_model_extracts_json_result() {
        let raw = serde_json::json!({
            "result": "Found compact trigger in auto_compact.rs",
            "steps": []
        })
        .to_string();

        assert_eq!(
            delegate_result_for_model(&raw),
            "Found compact trigger in auto_compact.rs"
        );
    }

    #[test]
    fn join_error_records_failed_task() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            crate::agent::a2a::types::AgentRole::Researcher,
            crate::agent::a2a::types::AgentExecutionMode::ReadOnly,
            "Read files",
            "Read files",
            10,
        );

        record_child_failure(&mut bus, &task_id, "join_error", "subagent panicked", 20);

        let projection = bus.projection();
        assert_eq!(projection.failed_count, 1);
        assert_eq!(
            projection.tasks[0].failure_message.as_deref(),
            Some("subagent panicked")
        );
    }

    #[test]
    fn extract_patch_proposal_from_json_block() {
        let raw = r#"
Here is my analysis.

```json
{
  "result": "Add null check",
  "patch_proposal": {
    "file_path": "src/main.rs",
    "intent": "Prevent panic on null input",
    "diff_summary": "Add early return for null",
    "original_snippet": "fn handle(x: Option<T>) { x.unwrap() }",
    "proposed_snippet": "fn handle(x: Option<T>) { x? }",
    "risk_level": "low",
    "test_suggestion": "Test with None",
    "confidence": 0.9
  }
}
```
"#;

        let proposal = extract_patch_proposal(raw).expect("should extract");
        assert_eq!(proposal.file_path, "src/main.rs");
        assert_eq!(proposal.intent, "Prevent panic on null input");
        assert_eq!(
            proposal.risk_level,
            crate::agent::a2a::types::PatchRiskLevel::Low
        );
        assert!((proposal.confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn extract_patch_proposal_from_wrapped_sub_agent_result() {
        let result = r#"
Analysis.

```json
{
  "result": "Review only",
  "patch_proposal": {
    "file_path": "src/lib.rs",
    "intent": "Avoid duplicate artifact loss",
    "diff_summary": "Parse nested patch proposal output",
    "original_snippet": "extract_patch_proposal(&raw)",
    "proposed_snippet": "extract_patch_proposal(&delegate_result_for_model(&raw))",
    "risk_level": "medium",
    "test_suggestion": "Run patch proposal extraction tests",
    "confidence": 0.8
  }
}
```
"#;
        let raw = serde_json::json!({
            "result": result,
            "steps": []
        })
        .to_string();

        let proposal = extract_patch_proposal(&raw).expect("should extract from wrapped result");
        assert_eq!(proposal.file_path, "src/lib.rs");
        assert_eq!(
            proposal.risk_level,
            crate::agent::a2a::types::PatchRiskLevel::Medium
        );
        assert!((proposal.confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn extract_patch_proposal_returns_none_for_missing_block() {
        let raw = "Just plain text result without any JSON.";
        assert!(extract_patch_proposal(raw).is_none());
    }

    #[test]
    fn extract_patch_proposal_returns_none_for_json_without_patch_proposal_key() {
        let raw = r#"{"result": "nothing here"}"#;
        assert!(extract_patch_proposal(raw).is_none());
    }

    #[test]
    fn assign_patch_proposal_task_uses_implementer_role() {
        let mut bus = AgentA2ABus::default();
        let task_id = assign_patch_proposal_task(&mut bus, "Fix bug", "Handle null", 10);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.role, crate::agent::a2a::types::AgentRole::Implementer);
        assert_eq!(
            task.execution_mode,
            crate::agent::a2a::types::AgentExecutionMode::PatchProposal
        );
    }

    #[test]
    fn assign_worktree_worker_task_uses_worktree_mode() {
        let mut bus = AgentA2ABus::default();
        let task_id = assign_worktree_worker_task(&mut bus, "Implement feature", "Add auth", 10);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.role, crate::agent::a2a::types::AgentRole::Implementer);
        assert_eq!(
            task.execution_mode,
            crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker
        );
        assert!(task.permissions.allow_workspace_write);
        assert!(task.permissions.allow_shell);
        assert!(!task.permissions.allow_delegate);
    }

    #[test]
    fn worktree_result_for_model_extracts_json_result() {
        let worker_result = serde_json::json!({
            "result": "Implemented login flow",
            "steps": []
        })
        .to_string();
        let raw = serde_json::json!({
            "result": worker_result,
            "diff": "diff --git a/src/auth.rs",
            "diff_available": true,
            "diff_truncated": false,
            "test_report": "2 passed",
            "tests_passed": true,
            "needs_human_review": true,
            "suggested_action": "Please review before merging.",
            "reason_codes": [],
            "worktree_path": "/tmp/wt",
            "cleaned_up": true
        })
        .to_string();

        let result = worktree_result_for_model(&raw);
        assert!(result.contains("Implemented login flow"));
        assert!(result.contains("Diff available: true"));
        assert!(result.contains("Worktree cleaned up: true"));
        assert!(result.contains("Test report: 2 passed"));
        assert!(result.contains("HUMAN REVIEW REQUIRED"));
        assert!(result.contains("Tests: PASSED"));
    }

    #[test]
    fn worktree_result_for_model_fallback_to_raw() {
        let raw = "Plain text result without JSON";
        assert_eq!(worktree_result_for_model(raw), raw);
    }

    #[test]
    fn extract_worktree_artifacts_produces_diff_and_test_report() {
        let task_id = AgentTaskId::new("wt-1");
        let raw = serde_json::json!({
            "result": "Done",
            "diff": "diff --git a/src/lib.rs",
            "diff_available": true,
            "diff_truncated": false,
            "test_report": "5 passed, 0 failed",
            "tests_passed": true,
            "needs_human_review": true,
            "suggested_action": "Review before merge.",
            "reason_codes": ["diff was truncated"],
            "worktree_path": "/tmp/wt",
            "cleaned_up": true
        })
        .to_string();

        let artifacts = extract_worktree_artifacts(&raw, &task_id);

        assert_eq!(artifacts.len(), 3);
        assert_eq!(
            artifacts[0].kind,
            crate::agent::a2a::types::AgentArtifactKind::DiffSummary
        );
        assert!(artifacts[0].content.contains("diff --git"));
        assert_eq!(
            artifacts[1].kind,
            crate::agent::a2a::types::AgentArtifactKind::TestReport
        );
        assert_eq!(artifacts[1].content, "5 passed, 0 failed");
        assert_eq!(
            artifacts[2].kind,
            crate::agent::a2a::types::AgentArtifactKind::Evidence
        );
        let meta = &artifacts[2].content;
        assert!(meta.contains("worktree_path"));
        assert!(meta.contains("needs_human_review"));
        assert!(meta.contains("tests_passed"));
        assert!(meta.contains("reason_codes"));
        assert!(meta.contains("diff was truncated"));
    }

    #[test]
    fn arbitrate_single_worker_with_all_pass() {
        let summaries = vec![WorktreeWorkerSummary {
            result: "Done".to_string(),
            diff: Some("diff".to_string()),
            diff_available: true,
            diff_truncated: false,
            test_report: Some("5 passed".to_string()),
            tests_passed: Some(true),
            needs_human_review: true,
            suggested_action: "Review".to_string(),
            reason_codes: vec![],
            worktree_path: "/tmp/wt".to_string(),
            cleaned_up: true,
        }];
        let arb = arbitrate_worktree_workers(&summaries);
        assert!(arb.needs_human_review);
        assert!(!arb.can_propose_apply);
        assert!(!arb.conflict_detected);
        assert_eq!(arb.worker_count, 1);
        assert_eq!(arb.workers_with_tests_passed, 1);
        assert!(arb.suggested_action.contains("HUMAN REVIEW REQUIRED"));
    }

    #[test]
    fn arbitrate_multiple_workers_detects_conflicts() {
        let summaries = vec![
            WorktreeWorkerSummary {
                result: "Done".to_string(),
                diff: Some("diff".to_string()),
                diff_available: true,
                diff_truncated: false,
                test_report: Some("5 passed".to_string()),
                tests_passed: Some(true),
                needs_human_review: true,
                suggested_action: "Review".to_string(),
                reason_codes: vec![],
                worktree_path: "/tmp/wt1".to_string(),
                cleaned_up: true,
            },
            WorktreeWorkerSummary {
                result: "Done".to_string(),
                diff: Some("diff".to_string()),
                diff_available: true,
                diff_truncated: false,
                test_report: Some("3 passed, 2 failed".to_string()),
                tests_passed: Some(false),
                needs_human_review: true,
                suggested_action: "Review".to_string(),
                reason_codes: vec!["tests failed".to_string()],
                worktree_path: "/tmp/wt2".to_string(),
                cleaned_up: true,
            },
        ];
        let arb = arbitrate_worktree_workers(&summaries);
        assert!(arb.conflict_detected);
        assert!(arb
            .aggregated_reasons
            .contains(&"Workers report inconsistent test results".to_string()));
        assert!(arb
            .aggregated_reasons
            .contains(&"1 worker(s) had failing tests".to_string()));
        assert!(arb.suggested_action.contains("conflicting results"));
        assert_eq!(arb.workers_with_tests_passed, 1);
    }

    #[test]
    fn arbitrate_flags_truncated_and_no_diff() {
        let summaries = vec![
            WorktreeWorkerSummary {
                result: "Done".to_string(),
                diff: Some("truncated".to_string()),
                diff_available: true,
                diff_truncated: true,
                test_report: None,
                tests_passed: None,
                needs_human_review: true,
                suggested_action: "Review".to_string(),
                reason_codes: vec!["diff was truncated".to_string()],
                worktree_path: "/tmp/wt1".to_string(),
                cleaned_up: false,
            },
            WorktreeWorkerSummary {
                result: "Done".to_string(),
                diff: None,
                diff_available: false,
                diff_truncated: false,
                test_report: None,
                tests_passed: None,
                needs_human_review: true,
                suggested_action: "Review".to_string(),
                reason_codes: vec!["no diff was produced".to_string()],
                worktree_path: "/tmp/wt2".to_string(),
                cleaned_up: true,
            },
        ];
        let arb = arbitrate_worktree_workers(&summaries);
        assert!(!arb.can_propose_apply);
        assert!(arb
            .aggregated_reasons
            .contains(&"1 worker(s) had truncated diffs".to_string()));
        assert!(arb
            .aggregated_reasons
            .contains(&"1 worker(s) produced no diff".to_string()));
    }

    #[test]
    fn apply_contract_blocks_everything_in_phase_6() {
        let summary = WorktreeWorkerSummary {
            result: "Done".to_string(),
            diff: Some("diff".to_string()),
            diff_available: true,
            diff_truncated: false,
            test_report: Some("5 passed".to_string()),
            tests_passed: Some(true),
            needs_human_review: true,
            suggested_action: "Review".to_string(),
            reason_codes: vec![],
            worktree_path: "/tmp/wt".to_string(),
            cleaned_up: false,
        };
        let proposal = can_propose_apply(&summary);
        assert!(!proposal.can_propose, "Phase 6 should never allow propose");
        assert!(proposal.user_confirmation_required);
        assert!(
            proposal.blockers.is_empty(),
            "perfect summary has no blockers"
        );
    }

    #[test]
    fn apply_contract_detects_blockers() {
        let summary = WorktreeWorkerSummary {
            result: "Done".to_string(),
            diff: Some("truncated".to_string()),
            diff_available: true,
            diff_truncated: true,
            test_report: Some("3 passed, 2 failed".to_string()),
            tests_passed: Some(false),
            needs_human_review: true,
            suggested_action: "Review".to_string(),
            reason_codes: vec!["tests failed".to_string(), "diff was truncated".to_string()],
            worktree_path: "/tmp/wt".to_string(),
            cleaned_up: true,
        };
        let proposal = can_propose_apply(&summary);
        assert!(!proposal.can_propose);
        assert!(proposal
            .blockers
            .contains(&"tests did not pass".to_string()));
        assert!(proposal
            .blockers
            .contains(&"diff was truncated".to_string()));
        assert!(proposal
            .blockers
            .contains(&"worktree was already cleaned up".to_string()));
        assert!(
            proposal
                .blockers
                .iter()
                .any(|b| b.contains("high-risk reason")),
            "should flag high-risk reasons"
        );
    }

    #[test]
    fn extract_worktree_artifacts_returns_empty_for_invalid_json() {
        let task_id = AgentTaskId::new("wt-2");
        assert!(extract_worktree_artifacts("not json", &task_id).is_empty());
    }
}
