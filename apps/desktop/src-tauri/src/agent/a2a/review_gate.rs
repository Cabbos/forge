use crate::agent::a2a::worktree::TestReport;

/// Reason codes describing why human review is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewReason {
    /// Diff was truncated — we don't know the full scope of changes.
    DiffTruncated,
    /// Tests failed — changes are likely broken.
    TestsFailed,
    /// We saw test-related output but could not parse a structured report.
    TestReportUnparseable,
    /// The sub-agent itself reported an error during execution.
    SubAgentError,
    /// No diff was produced — the worker may not have made any changes.
    NoDiffProduced,
}

impl ReviewReason {
    pub fn description(&self) -> &'static str {
        match self {
            Self::DiffTruncated => "diff was truncated",
            Self::TestsFailed => "tests failed",
            Self::TestReportUnparseable => "test report could not be parsed",
            Self::SubAgentError => "sub-agent reported an error",
            Self::NoDiffProduced => "no diff was produced",
        }
    }
}

/// The result of the review-gate evaluation for a WorktreeWorker run.
///
/// WorktreeWorker **always** requires human review — this struct makes the
/// *reasons* explicit so the parent agent and UI can surface the right
/// safety signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewGateDecision {
    /// Always `true` for WorktreeWorker (never auto-merge).
    pub needs_human_review: bool,
    /// Human-readable suggested next step.
    pub suggested_action: String,
    /// Whether the worktree should be preserved for inspection.
    pub preserve_worktree: bool,
    /// Specific reason codes for telemetry / UI badges.
    pub reason_codes: Vec<ReviewReason>,
}

impl ReviewGateDecision {
    fn build_suggested_action(&self, diff_available: bool, tests_passed: Option<bool>) -> String {
        let mut parts = Vec::new();

        // Lead with the safety assertion.
        parts.push("HUMAN REVIEW REQUIRED — do not merge automatically.".to_string());

        // Enumerate concrete reasons.
        if !self.reason_codes.is_empty() {
            let reasons: Vec<_> = self.reason_codes.iter().map(|r| r.description()).collect();
            parts.push(format!("Reason(s): {}.", reasons.join(", ")));
        }

        // Diff / test status summary.
        if diff_available {
            match tests_passed {
                Some(true) => parts.push("Diff is available and tests pass.".to_string()),
                Some(false) => parts.push("Diff is available but tests FAILED.".to_string()),
                None => parts.push("Diff is available but test status is unclear.".to_string()),
            }
        } else {
            parts.push("No diff produced — verify the worker actually made changes.".to_string());
        }

        if self.preserve_worktree {
            parts.push("Worktree preserved for inspection.".to_string());
        }

        parts.join("\n")
    }
}

/// Compute the review-gate decision for a WorktreeWorker result.
///
/// This is a pure function — all inputs are simple types — so it can be
/// exhaustively unit-tested without spawning git worktrees or model adapters.
pub(crate) fn compute_review_gate(
    diff_available: bool,
    diff_truncated: bool,
    test_report_structured: Option<&TestReport>,
    tests_passed: Option<bool>,
    sub_has_error: bool,
    sub_result: &str,
) -> ReviewGateDecision {
    let mut reasons = Vec::new();
    let mut preserve = false;

    // Safety invariant: WorktreeWorker never auto-merges.
    let needs_human_review = true;

    // 1. Diff truncated → we cannot assess the full change surface.
    if diff_truncated {
        reasons.push(ReviewReason::DiffTruncated);
        preserve = true;
    }

    // 2. Tests failed → code is likely broken.
    if tests_passed == Some(false) {
        reasons.push(ReviewReason::TestsFailed);
        preserve = true;
    }

    // 3. Test report unparseable → we cannot verify quality.
    //    Only flag this if the result mentions tests but we couldn't structure them.
    if test_report_structured.is_none() {
        let lower = sub_result.to_lowercase();
        if lower.contains("test")
            || lower.contains("cargo test")
            || lower.contains("npm test")
            || lower.contains("pytest")
        {
            reasons.push(ReviewReason::TestReportUnparseable);
        }
    }

    // 4. Sub-agent error → execution may be incomplete.
    if sub_has_error {
        reasons.push(ReviewReason::SubAgentError);
        preserve = true;
    }

    // 5. No diff → worker may not have actually changed anything.
    if !diff_available {
        reasons.push(ReviewReason::NoDiffProduced);
    }

    let mut decision = ReviewGateDecision {
        needs_human_review,
        suggested_action: String::new(),
        preserve_worktree: preserve,
        reason_codes: reasons,
    };
    decision.suggested_action = decision.build_suggested_action(diff_available, tests_passed);
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_still_requires_review() {
        let decision = compute_review_gate(
            true,  // diff available
            false, // not truncated
            Some(&TestReport {
                passed: 5,
                failed: 0,
                skipped: 0,
                exit_code: Some(0),
                summary: "5 passed, 0 failed".to_string(),
            }),
            Some(true), // tests pass
            false,      // no error
            "Done",
        );
        assert!(decision.needs_human_review);
        assert!(!decision.preserve_worktree);
        assert!(decision.suggested_action.contains("HUMAN REVIEW REQUIRED"));
        assert!(decision
            .suggested_action
            .contains("Diff is available and tests pass"));
        assert!(decision.reason_codes.is_empty());
    }

    #[test]
    fn diff_truncated_flags_preserve() {
        let decision = compute_review_gate(
            true,
            true, // truncated
            Some(&TestReport {
                passed: 5,
                failed: 0,
                skipped: 0,
                exit_code: Some(0),
                summary: "5 passed".to_string(),
            }),
            Some(true),
            false,
            "Done",
        );
        assert!(decision.needs_human_review);
        assert!(decision.preserve_worktree);
        assert!(decision.reason_codes.contains(&ReviewReason::DiffTruncated));
        assert!(decision.suggested_action.contains("diff was truncated"));
        assert!(decision.suggested_action.contains("Worktree preserved"));
    }

    #[test]
    fn tests_failed_flags_preserve() {
        let decision = compute_review_gate(
            true,
            false,
            Some(&TestReport {
                passed: 3,
                failed: 2,
                skipped: 0,
                exit_code: Some(1),
                summary: "3 passed, 2 failed".to_string(),
            }),
            Some(false), // tests failed
            false,
            "Done",
        );
        assert!(decision.preserve_worktree);
        assert!(decision.reason_codes.contains(&ReviewReason::TestsFailed));
        assert!(decision.suggested_action.contains("tests failed"));
        assert!(decision.suggested_action.contains("tests FAILED"));
    }

    #[test]
    fn sub_agent_error_flags_preserve() {
        let decision = compute_review_gate(
            true, false, None, None, true, // sub has error
            "Panicked",
        );
        assert!(decision.preserve_worktree);
        assert!(decision.reason_codes.contains(&ReviewReason::SubAgentError));
        assert!(decision
            .suggested_action
            .contains("sub-agent reported an error"));
    }

    #[test]
    fn no_diff_produced_is_flagged() {
        let decision = compute_review_gate(
            false, // no diff
            false, None, None, false, "Done",
        );
        assert!(!decision.preserve_worktree);
        assert!(decision
            .reason_codes
            .contains(&ReviewReason::NoDiffProduced));
        assert!(decision.suggested_action.contains("No diff produced"));
    }

    #[test]
    fn unparseable_test_report_with_test_mention() {
        let decision = compute_review_gate(
            true,
            false,
            None, // unparseable
            None,
            false,
            "Ran cargo test but output was weird",
        );
        assert!(decision
            .reason_codes
            .contains(&ReviewReason::TestReportUnparseable));
        assert!(decision
            .suggested_action
            .contains("test report could not be parsed"));
    }

    #[test]
    fn parseable_test_report_no_flag() {
        let decision = compute_review_gate(
            true,
            false,
            Some(&TestReport {
                passed: 5,
                failed: 0,
                skipped: 0,
                exit_code: Some(0),
                summary: "5 passed".to_string(),
            }),
            Some(true),
            false,
            "Ran cargo test but output was weird", // text mentions test
        );
        // Because structured report IS present, we do NOT flag unparseable.
        assert!(!decision
            .reason_codes
            .contains(&ReviewReason::TestReportUnparseable));
    }

    #[test]
    fn no_test_mention_no_unparseable_flag() {
        let decision = compute_review_gate(true, false, None, None, false, "Fixed typo in README");
        assert!(!decision
            .reason_codes
            .contains(&ReviewReason::TestReportUnparseable));
    }

    #[test]
    fn combined_failure_reasons_all_present() {
        let decision = compute_review_gate(
            false, // no diff
            true,  // truncated
            None,
            Some(false), // tests failed
            true,        // sub error
            "Error running npm test",
        );
        assert!(decision.preserve_worktree);
        assert_eq!(decision.reason_codes.len(), 5);
        assert!(decision.reason_codes.contains(&ReviewReason::DiffTruncated));
        assert!(decision.reason_codes.contains(&ReviewReason::TestsFailed));
        assert!(decision.reason_codes.contains(&ReviewReason::SubAgentError));
        assert!(decision
            .reason_codes
            .contains(&ReviewReason::NoDiffProduced));
        // "test" is in the text but structured report is None — should flag unparseable too.
        assert!(decision
            .reason_codes
            .contains(&ReviewReason::TestReportUnparseable));
    }

    #[test]
    fn suggested_action_never_mentions_auto_merge() {
        // Defensive: ensure no wording accidentally suggests automatic merge.
        let decision = compute_review_gate(true, false, None, Some(true), false, "Done");
        let action_lower = decision.suggested_action.to_lowercase();
        assert!(
            !action_lower.contains("auto merge"),
            "suggested_action should never mention auto-merge"
        );
        assert!(
            !action_lower.contains("automatically merge"),
            "suggested_action should never mention automatically merge"
        );
        assert!(
            action_lower.contains("human review"),
            "suggested_action must mention human review"
        );
    }
}
