use crate::agent::turn_state::{AgentTurnStatus, AgentVerificationStatus, AgentVerificationTrace};

pub(crate) fn final_answer_instruction(verification: Option<&AgentVerificationTrace>) -> String {
    let Some(trace) = verification.filter(|trace| verification_has_failed(trace)) else {
        return "Based on the above, provide your final answer as plain text. Do not use tools."
            .to_string();
    };

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
        final_turn_status_for_current_turn, final_turn_status_for_run,
        final_turn_transition_reason_for_current_turn, final_turn_transition_reason_for_run,
        verification_has_failed,
    };
    use crate::agent::turn_state::{
        AgentTurnStatus, AgentVerificationStatus, AgentVerificationTrace,
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
}
