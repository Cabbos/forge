use crate::agent::turn_state::{
    AgentEvidenceKind, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};

pub(crate) fn final_answer_instruction(
    verification: Option<&AgentVerificationTrace>,
    latest_turn: Option<&AgentTurnState>,
) -> String {
    let mut instruction = if let Some(trace) =
        verification.filter(|trace| verification_has_failed(trace))
    {
        failed_verification_final_answer_instruction(trace)
    } else {
        "Based on the above, provide your final answer as plain text. Do not use tools.".to_string()
    };

    append_preview_ownership_instruction(&mut instruction, latest_turn);
    instruction
}

fn failed_verification_final_answer_instruction(trace: &AgentVerificationTrace) -> String {
    let mut detail = String::from(
        "Based on the above, provide your final answer as plain text. Do not use tools. Verification did not pass, so clearly tell the user what failed and avoid claiming the task is fully complete.",
    );
    if let Some(command) = trace.command.as_deref() {
        detail.push_str(&format!("\nVerification command: {command}"));
    }
    if let Some(exit_code) = trace.exit_code {
        detail.push_str(&format!("\nExit code: {exit_code}"));
    }
    if let Some(stderr) = trace.stderr_preview.as_deref() {
        detail.push_str(&format!("\nError output: {stderr}"));
    }
    if let Some(stdout) = trace.stdout_preview.as_deref() {
        detail.push_str(&format!("\nOutput: {stdout}"));
    }
    detail
}

fn append_preview_ownership_instruction(
    instruction: &mut String,
    latest_turn: Option<&AgentTurnState>,
) {
    let Some(summary) = latest_preview_evidence_summary(latest_turn) else {
        return;
    };

    instruction.push_str("\nPreview ownership evidence:\n");
    instruction.push_str(summary);
    instruction.push_str("\nIf your final answer mentions a preview URL, explicitly say whether it belongs to the current project/workspace based on this evidence. If the evidence indicates a conflict or missing owner, state that clearly instead of leaving ownership implicit.");
}

fn latest_preview_evidence_summary(latest_turn: Option<&AgentTurnState>) -> Option<&str> {
    latest_turn?
        .evidence
        .iter()
        .rev()
        .find(|evidence| evidence.kind == AgentEvidenceKind::Preview)
        .and_then(|evidence| evidence.summary.as_deref())
        .filter(|summary| !summary.trim().is_empty())
}

pub(crate) fn final_turn_status_for_run(
    running: bool,
    verification: Option<&AgentVerificationTrace>,
) -> AgentTurnStatus {
    if !running {
        return AgentTurnStatus::Cancelled;
    }
    if verification.is_some_and(verification_has_failed) {
        AgentTurnStatus::Failed
    } else {
        AgentTurnStatus::Completed
    }
}

pub(crate) fn final_turn_status_for_current_turn(
    current_status: AgentTurnStatus,
    running: bool,
    verification: Option<&AgentVerificationTrace>,
) -> AgentTurnStatus {
    if current_status == AgentTurnStatus::Cancelled {
        return AgentTurnStatus::Cancelled;
    }
    final_turn_status_for_run(running, verification)
}

pub(crate) fn final_turn_transition_reason_for_run(
    running: bool,
    verification: Option<&AgentVerificationTrace>,
) -> &'static str {
    if !running {
        return "user_cancelled";
    }
    if verification.is_some_and(verification_has_failed) {
        "verification_failed"
    } else {
        "final_answer"
    }
}

pub(crate) fn final_turn_transition_reason_for_current_turn(
    current_status: AgentTurnStatus,
    running: bool,
    verification: Option<&AgentVerificationTrace>,
) -> &'static str {
    if current_status == AgentTurnStatus::Cancelled {
        return "user_cancelled";
    }
    final_turn_transition_reason_for_run(running, verification)
}

pub(crate) fn verification_has_failed(trace: &AgentVerificationTrace) -> bool {
    matches!(
        trace.status,
        AgentVerificationStatus::Failed | AgentVerificationStatus::Error
    )
}

#[cfg(test)]
mod tests {
    use super::{
        final_answer_instruction, final_turn_status_for_current_turn, final_turn_status_for_run,
        final_turn_transition_reason_for_current_turn, final_turn_transition_reason_for_run,
        verification_has_failed,
    };
    use crate::agent::turn_state::{
        AgentTurnState, AgentTurnStatus, AgentVerificationStatus, AgentVerificationTrace,
    };

    fn verification(status: AgentVerificationStatus) -> AgentVerificationTrace {
        AgentVerificationTrace {
            status,
            command: Some("npm run build".to_string()),
            exit_code: Some(1),
            stdout_preview: None,
            stderr_preview: Some("build failed".to_string()),
            duration_ms: Some(10),
            completed_at_ms: Some(20),
        }
    }

    fn turn_with_running_preview() -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/Users/cabbos/project/forge-test-app".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "chat".to_string(),
            "working".to_string(),
            "fix the demo button".to_string(),
        );
        turn.record_preview_status(
            Some("/Users/cabbos/project/forge-test-app"),
            true,
            false,
            true,
            "Preview is running",
            Some("http://127.0.0.1:5173"),
        );
        turn
    }

    #[test]
    fn failed_verification_keeps_turn_failed() {
        let trace = verification(AgentVerificationStatus::Failed);

        assert_eq!(
            final_turn_status_for_run(true, Some(&trace)),
            AgentTurnStatus::Failed
        );
    }

    #[test]
    fn error_verification_keeps_turn_failed() {
        let trace = verification(AgentVerificationStatus::Error);

        assert_eq!(
            final_turn_status_for_run(true, Some(&trace)),
            AgentTurnStatus::Failed
        );
    }

    #[test]
    fn passed_verification_allows_turn_completed() {
        let trace = verification(AgentVerificationStatus::Passed);

        assert_eq!(
            final_turn_status_for_run(true, Some(&trace)),
            AgentTurnStatus::Completed
        );
    }

    #[test]
    fn stopped_run_marks_turn_cancelled() {
        assert_eq!(
            final_turn_status_for_run(false, None),
            AgentTurnStatus::Cancelled
        );
    }

    #[test]
    fn cancelled_current_turn_stays_cancelled_even_if_session_resumed_before_finalization() {
        assert_eq!(
            final_turn_status_for_current_turn(
                AgentTurnStatus::Cancelled,
                true,
                Some(&verification(AgentVerificationStatus::Passed)),
            ),
            AgentTurnStatus::Cancelled
        );
        assert_eq!(
            final_turn_transition_reason_for_current_turn(
                AgentTurnStatus::Cancelled,
                true,
                Some(&verification(AgentVerificationStatus::Passed)),
            ),
            "user_cancelled"
        );
    }

    #[test]
    fn final_transition_reason_explains_completion_failure_and_cancel() {
        let failed = verification(AgentVerificationStatus::Failed);
        let passed = verification(AgentVerificationStatus::Passed);

        assert_eq!(
            final_turn_transition_reason_for_run(true, Some(&failed)),
            "verification_failed"
        );
        assert_eq!(
            final_turn_transition_reason_for_run(true, Some(&passed)),
            "final_answer"
        );
        assert_eq!(
            final_turn_transition_reason_for_run(false, None),
            "user_cancelled"
        );
    }

    #[test]
    fn verification_failed_matches_failed_and_error_only() {
        assert!(verification_has_failed(&verification(
            AgentVerificationStatus::Failed
        )));
        assert!(verification_has_failed(&verification(
            AgentVerificationStatus::Error
        )));
        assert!(!verification_has_failed(&verification(
            AgentVerificationStatus::Passed
        )));
    }

    #[test]
    fn final_answer_instruction_includes_preview_ownership_evidence_when_present() {
        let turn = turn_with_running_preview();

        let instruction = final_answer_instruction(None, Some(&turn));

        assert!(instruction.contains("Preview ownership evidence"));
        assert!(instruction.contains("project_path=/Users/cabbos/project/forge-test-app"));
        assert!(instruction.contains("url=http://127.0.0.1:5173"));
        assert!(instruction.contains("explicitly say whether it belongs to the current project"));
    }

    #[test]
    fn final_answer_instruction_keeps_failed_verification_and_preview_ownership_guidance() {
        let turn = turn_with_running_preview();
        let trace = verification(AgentVerificationStatus::Failed);

        let instruction = final_answer_instruction(Some(&trace), Some(&turn));

        assert!(instruction.contains("Verification did not pass"));
        assert!(instruction.contains("Verification command: npm run build"));
        assert!(instruction.contains("Preview ownership evidence"));
        assert!(instruction.contains("project_path=/Users/cabbos/project/forge-test-app"));
    }

    #[test]
    fn final_answer_instruction_omits_preview_ownership_without_preview_evidence() {
        let instruction = final_answer_instruction(None, None);

        assert!(!instruction.contains("Preview ownership evidence"));
        assert!(instruction.contains("provide your final answer as plain text"));
    }
}
