use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::a2a::types::{AgentExecutionMode, AgentRole, AgentTaskId, PatchProposal};

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

    // Worktree metadata artifact (path, cleanup status)
    if let Ok(summary) =
        serde_json::from_str::<crate::agent::a2a::worktree::WorktreeWorkerSummary>(raw)
    {
        let meta = serde_json::json!({
            "worktree_path": summary.worktree_path,
            "cleaned_up": summary.cleaned_up,
            "diff_available": summary.diff_available,
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
        let raw = serde_json::json!({
            "result": "Implemented login flow",
            "diff": "diff --git a/src/auth.rs",
            "cleaned_up": true
        })
        .to_string();

        assert_eq!(worktree_result_for_model(&raw), "Implemented login flow");
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
            "test_report": "5 passed, 0 failed",
            "worktree_path": "/tmp/wt",
            "cleaned_up": true,
            "diff_available": true
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
        assert!(artifacts[2].content.contains("worktree_path"));
    }

    #[test]
    fn extract_worktree_artifacts_returns_empty_for_invalid_json() {
        let task_id = AgentTaskId::new("wt-2");
        assert!(extract_worktree_artifacts("not json", &task_id).is_empty());
    }
}
