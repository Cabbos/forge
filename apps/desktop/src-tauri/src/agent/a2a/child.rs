use std::path::Path;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::adapters::base::AiAdapter;
use crate::agent::event_sink::EventEmitter;
use crate::harness::Harness;

pub(crate) struct ChildAgentRuntime;

impl ChildAgentRuntime {
    pub(crate) async fn run_read_only(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_with_emitter(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }

    pub(crate) async fn run_patch_proposal(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_patch_proposal(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }

    /// Run a worktree-worker sub-agent in an isolated git worktree.
    ///
    /// The worker gets a fresh Harness pointing at the worktree, so it can
    /// write files and run shell commands without touching the main workspace.
    /// On completion we collect the git diff (and any test output) and return
    /// a structured summary to the parent model.
    pub(crate) async fn run_worktree_worker(
        worktree_id: &str,
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        parent_harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        use crate::agent::a2a::worktree::{LeaseResult, WorktreeLease, WorktreeWorkerSummary};

        let mut lease = match WorktreeLease::create(working_dir, worktree_id) {
            LeaseResult::Ok(l) => l,
            LeaseResult::NotAGitRepo { path } => {
                return serde_json::to_string(&WorktreeWorkerSummary {
                    result: format!(
                        "Cannot create worktree: {} is not inside a git repository. \
                         Falling back to patch_proposal mode is recommended.",
                        path.display()
                    ),
                    diff: None,
                    diff_available: false,
                    diff_truncated: false,
                    test_report: None,
                    tests_passed: None,
                    needs_human_review: true,
                    suggested_action: "Use patch_proposal mode or manually apply changes."
                        .to_string(),
                    reason_codes: vec!["not_a_git_repo".to_string()],
                    worktree_path: path.to_string_lossy().to_string(),
                    cleaned_up: true,
                })
                .unwrap_or_else(|_| "Worktree creation failed".to_string());
            }
            LeaseResult::GitError { message } => {
                return serde_json::to_string(&WorktreeWorkerSummary {
                    result: format!("Worktree creation failed: {message}"),
                    diff: None,
                    diff_available: false,
                    diff_truncated: false,
                    test_report: None,
                    tests_passed: None,
                    needs_human_review: true,
                    suggested_action: "Check git status and retry.".to_string(),
                    reason_codes: vec!["git_error".to_string()],
                    worktree_path: working_dir.to_string_lossy().to_string(),
                    cleaned_up: true,
                })
                .unwrap_or_else(|_| "Worktree creation failed".to_string());
            }
            LeaseResult::AlreadyInUse { branch_name } => {
                return serde_json::to_string(&WorktreeWorkerSummary {
                    result: format!(
                        "Worktree creation failed: branch {branch_name} is already in use. \
                         Another worktree worker may be running for the same task."
                    ),
                    diff: None,
                    diff_available: false,
                    diff_truncated: false,
                    test_report: None,
                    tests_passed: None,
                    needs_human_review: true,
                    suggested_action: "HUMAN REVIEW REQUIRED - wait for the other worker to finish or use a unique task id.".to_string(),
                    reason_codes: vec!["already_in_use".to_string()],
                    worktree_path: working_dir.to_string_lossy().to_string(),
                    cleaned_up: true,
                })
                .unwrap_or_else(|_| "Worktree creation failed".to_string());
            }
        };

        let worktree_path = lease.path().to_path_buf();

        // Create a fresh harness for the worktree so the worker has full
        // tool access inside the isolated directory.
        let worktree_harness = Arc::new(Harness::new_with_pending(
            worktree_path.clone(),
            parent_harness.pending_confirms.clone(),
        ));

        // Run the sub-agent in worktree-worker mode.
        let sub_result = crate::agent::sub::SubAgent::run_worktree_worker(
            task,
            adapter,
            worktree_harness,
            emitter,
            cancel,
            &worktree_path,
        )
        .await;

        // Collect diff from the worktree (with size protection).
        let (diff, diff_truncated) = match lease.diff_truncated() {
            Ok(d) => {
                let truncated = d.contains("[diff truncated:");
                (Some(d), truncated)
            }
            Err(e) => (Some(format!("Diff extraction failed: {e}")), false),
        };
        let diff_available = diff
            .as_ref()
            .is_some_and(|d| !d.trim().is_empty() && !d.starts_with("Diff extraction failed"));

        // Extract structured test report from the sub-agent result.
        let structured_report = extract_structured_test_report(&sub_result);
        let test_report = structured_report
            .as_ref()
            .map(|r| r.summary.clone())
            .or_else(|| extract_test_report_heuristic(&sub_result));

        // Determine whether the sub-agent itself signalled failure.
        let sub_has_error = sub_result.contains("error") || sub_result.contains("Error");
        let tests_passed = structured_report
            .as_ref()
            .map(|r| r.failed == 0 && r.exit_code.is_none_or(|ec| ec == 0))
            .or_else(|| {
                test_report.as_ref().map(|tr| {
                    !tr.contains("failed") && !tr.contains("FAIL")
                        || tr.contains("0 failed")
                        || tr.contains("all passed")
                })
            });

        // Review gate: compute explicit safety decision.
        let gate = crate::agent::a2a::review_gate::compute_review_gate(
            diff_available,
            diff_truncated,
            structured_report.as_ref(),
            tests_passed,
            sub_has_error,
            &sub_result,
        );

        if gate.preserve_worktree {
            lease.preserve();
        }

        let cleaned_up = if lease.is_preserved() {
            false
        } else {
            lease.cleanup().is_ok()
        };

        let summary = WorktreeWorkerSummary {
            result: sub_result,
            diff,
            diff_available,
            diff_truncated,
            test_report,
            tests_passed,
            needs_human_review: gate.needs_human_review,
            suggested_action: gate.suggested_action,
            reason_codes: gate
                .reason_codes
                .iter()
                .map(|r| r.description().to_string())
                .collect(),
            worktree_path: worktree_path.to_string_lossy().to_string(),
            cleaned_up,
        };

        serde_json::to_string(&summary).unwrap_or_else(|_| {
            format!(
                "{{\"result\": \"{}\", \"diff_available\": false, \"cleaned_up\": {}}}",
                summary.result.replace('"', "\\\""),
                cleaned_up
            )
        })
    }
}

/// Structured test report with numeric fields.
fn extract_structured_test_report(raw: &str) -> Option<crate::agent::a2a::worktree::TestReport> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;

    // Look for an explicit structured test_report object.
    if let Some(obj) = value.get("test_report") {
        if let Some(report) = obj.as_str() {
            // Plain text test report: try to parse counts heuristically.
            return parse_test_counts(report);
        }
        // Try to read a structured JSON test report.
        let passed = obj.get("passed").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let failed = obj.get("failed").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let skipped = obj.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let exit_code = obj
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let summary = obj
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if passed > 0 || failed > 0 || skipped > 0 || !summary.is_empty() {
            return Some(crate::agent::a2a::worktree::TestReport {
                passed,
                failed,
                skipped,
                exit_code,
                summary: if summary.is_empty() {
                    format!("{passed} passed, {failed} failed, {skipped} skipped")
                } else {
                    summary
                },
            });
        }
    }

    // Fallback: if the overall result text contains test output, try parsing it.
    let result = value.get("result").and_then(|v| v.as_str())?;
    parse_test_counts(result)
}

/// Heuristic parser for "X passed, Y failed" style test output.
fn parse_test_counts(text: &str) -> Option<crate::agent::a2a::worktree::TestReport> {
    let text_lower = text.to_lowercase();

    // Try common patterns: "3 passed, 0 failed", "5 failed, 10 passed", etc.
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    // Scan adjacent tokens for NUMBER + KEYWORD pairs.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for window in tokens.windows(2) {
        let num_part = window[0].trim_end_matches(|c: char| !c.is_ascii_digit());
        if let Ok(n) = num_part.parse::<u32>() {
            let rest = window[1].to_lowercase();
            if rest.starts_with("passed") || rest.starts_with("pass") {
                passed = n;
            } else if rest.starts_with("failed") || rest.starts_with("fail") {
                failed = n;
            } else if rest.starts_with("skipped") || rest.starts_with("skip") {
                skipped = n;
            }
        }
    }

    // If we found no counts but the text looks like test output, still return a report.
    let is_test_output = text_lower.contains("test")
        || text_lower.contains("cargo test")
        || text_lower.contains("npm test")
        || text_lower.contains("pytest");

    if passed > 0 || failed > 0 || skipped > 0 || is_test_output {
        Some(crate::agent::a2a::worktree::TestReport {
            passed,
            failed,
            skipped,
            exit_code: None,
            summary: text.to_string(),
        })
    } else {
        None
    }
}

/// Legacy heuristic extraction, kept as fallback when structured parsing yields nothing.
fn extract_test_report_heuristic(raw: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    // If the result contains an explicit test_report field, use it.
    if let Some(report) = value.get("test_report").and_then(|v| v.as_str()) {
        if !report.is_empty() {
            return Some(report.to_string());
        }
    }
    // Otherwise, if the overall result text looks like it contains test output,
    // surface the whole result as the test report.
    let result = value.get("result").and_then(|v| v.as_str())?;
    if result.contains("test")
        || result.contains("Test")
        || result.contains("cargo test")
        || result.contains("npm test")
        || result.contains("pytest")
    {
        Some(result.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_structured_test_report_from_json_object() {
        let raw = r#"{"result": "Done", "test_report": {"passed": 3, "failed": 1, "skipped": 0, "exit_code": 0, "summary": "3 passed, 1 failed"}}"#;
        let report = extract_structured_test_report(raw).expect("should parse");
        assert_eq!(report.passed, 3);
        assert_eq!(report.failed, 1);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.exit_code, Some(0));
        assert_eq!(report.summary, "3 passed, 1 failed");
    }

    #[test]
    fn extract_structured_test_report_from_text_field() {
        let raw = r#"{"result": "Done", "test_report": "5 passed, 0 failed, 1 skipped"}"#;
        let report = extract_structured_test_report(raw).expect("should parse");
        assert_eq!(report.passed, 5);
        assert_eq!(report.failed, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn extract_structured_test_report_from_result_heuristic() {
        let raw = r#"{"result": "Ran cargo test. 5 passed, 1 failed."}"#;
        let report = extract_structured_test_report(raw).expect("should parse");
        assert_eq!(report.passed, 5);
        assert_eq!(report.failed, 1);
    }

    #[test]
    fn extract_structured_test_report_returns_none_when_no_tests() {
        let raw = r#"{"result": "Fixed typo in README"}"#;
        assert!(extract_structured_test_report(raw).is_none());
    }

    #[test]
    fn extract_test_report_heuristic_from_json_field() {
        let raw = r#"{"result": "Done", "test_report": "3 passed, 0 failed"}"#;
        assert_eq!(
            extract_test_report_heuristic(raw),
            Some("3 passed, 0 failed".to_string())
        );
    }

    #[test]
    fn extract_test_report_heuristic_from_result_heuristic() {
        let raw = r#"{"result": "Ran cargo test. 5 passed, 1 failed."}"#;
        assert_eq!(
            extract_test_report_heuristic(raw),
            Some("Ran cargo test. 5 passed, 1 failed.".to_string())
        );
    }

    #[test]
    fn extract_test_report_heuristic_returns_none_when_no_tests() {
        let raw = r#"{"result": "Fixed typo in README"}"#;
        assert_eq!(extract_test_report_heuristic(raw), None);
    }

    #[test]
    fn parse_test_counts_handles_various_formats() {
        let cases = [
            ("3 passed, 0 failed", 3, 0, 0),
            ("10 failed, 5 passed", 5, 10, 0),
            ("1 skipped, 2 passed, 3 failed", 2, 3, 1),
            ("cargo test: 7 passed; 2 failed", 7, 2, 0),
        ];
        for (text, expected_passed, expected_failed, expected_skipped) in cases {
            let report = parse_test_counts(text).expect("should parse");
            assert_eq!(
                report.passed, expected_passed,
                "passed mismatch for '{text}'"
            );
            assert_eq!(
                report.failed, expected_failed,
                "failed mismatch for '{text}'"
            );
            assert_eq!(
                report.skipped, expected_skipped,
                "skipped mismatch for '{text}'"
            );
        }
    }

    #[test]
    fn parse_test_counts_returns_none_for_plain_text() {
        assert!(parse_test_counts("Just plain text").is_none());
    }

    // Integration test: full worktree worker end-to-end.

    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockAdapter {
        calls: AtomicUsize,
    }

    fn init_test_repo(prefix: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(&tmp)
            .output()
            .expect("git init");
        assert!(init.status.success());

        std::fs::write(tmp.join("README.md"), "# test").unwrap();
        let add = Command::new("git")
            .args(["add", "."])
            .current_dir(&tmp)
            .output()
            .expect("git add");
        assert!(add.status.success());
        let commit = Command::new("git")
            .args(["commit", "-m", "init", "--no-gpg-sign"])
            .current_dir(&tmp)
            .output()
            .expect("git commit");
        assert!(commit.status.success());

        tmp
    }

    struct AutoApprovePendingEmitter {
        pending_confirms: Arc<
            tokio::sync::RwLock<
                std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>,
            >,
        >,
    }

    impl AutoApprovePendingEmitter {
        fn new(
            pending_confirms: Arc<
                tokio::sync::RwLock<
                    std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>,
                >,
            >,
        ) -> Self {
            Self { pending_confirms }
        }
    }

    impl EventEmitter for AutoApprovePendingEmitter {
        fn emit(&self, event: crate::protocol::events::StreamEvent) {
            if let crate::protocol::events::StreamEvent::ConfirmAsk { block_id, .. } = event {
                let pending_confirms = self.pending_confirms.clone();
                tokio::spawn(async move {
                    for _ in 0..100 {
                        if let Some(sender) = pending_confirms.write().await.remove(&block_id) {
                            let _ = sender.send(true);
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                });
            }
        }
    }

    #[async_trait::async_trait]
    impl AiAdapter for MockAdapter {
        async fn call(
            &self,
            _messages: &[crate::adapters::base::ChatMessage],
            _cancel: Arc<Notify>,
        ) -> Result<crate::adapters::base::StreamResult, crate::adapters::base::AdapterError>
        {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            match idx {
                0 => {
                    // Round 1: request a shell command to create a file in the worktree.
                    Ok(crate::adapters::base::StreamResult {
                        assistant_content: vec![serde_json::json!({
                            "type": "text",
                            "text": "Creating file..."
                        })],
                        tool_calls: vec![crate::adapters::base::ToolCall {
                            id: "call_1".to_string(),
                            name: "run_shell".to_string(),
                            input: serde_json::json!({
                                "command": "echo 'hello from worktree worker' > output.txt",
                                "timeout": 5
                            }),
                        }],
                        stop_reason: Some("tool_use".to_string()),
                    })
                }
                1 => {
                    // Round 2: final answer with test report.
                    Ok(crate::adapters::base::StreamResult {
                        assistant_content: vec![serde_json::json!({
                            "type": "text",
                            "text": "Done. Added output.txt and ran tests."
                        })],
                        tool_calls: vec![],
                        stop_reason: Some("end_turn".to_string()),
                    })
                }
                _ => Ok(crate::adapters::base::StreamResult {
                    assistant_content: vec![],
                    tool_calls: vec![],
                    stop_reason: Some("end_turn".to_string()),
                }),
            }
        }

        fn model_id(&self) -> &str {
            "mock"
        }

        fn model_name(&self) -> &str {
            "Mock"
        }
    }

    #[tokio::test]
    async fn run_worktree_worker_creates_worktree_collects_diff_and_returns_summary() {
        let tmp = init_test_repo("forge-test-wt-integration");

        let adapter: Arc<dyn AiAdapter> = Arc::new(MockAdapter {
            calls: AtomicUsize::new(0),
        });
        let pending_confirms = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let harness = Arc::new(Harness::new_with_pending(
            tmp.clone(),
            pending_confirms.clone(),
        ));
        let emitter: Arc<dyn EventEmitter> =
            Arc::new(AutoApprovePendingEmitter::new(pending_confirms));
        let cancel = Arc::new(Notify::new());

        let raw = ChildAgentRuntime::run_worktree_worker(
            "integration-task",
            "Write output.txt and run tests",
            adapter,
            harness,
            emitter,
            cancel,
            &tmp,
        )
        .await;

        // Parse the returned summary.
        let summary: crate::agent::a2a::worktree::WorktreeWorkerSummary =
            serde_json::from_str(&raw).expect("should parse WorktreeWorkerSummary");

        // Worktree should have been created. On success it should be cleaned up.
        assert!(
            summary.cleaned_up,
            "worktree should be cleaned up on success, got: {}",
            raw
        );
        // Diff may contain the new file or just untracked files from Harness.
        assert!(
            summary.diff_available,
            "diff should be available (either tracked changes or untracked files), got: {}",
            raw
        );
        assert!(
            summary.needs_human_review,
            "should always require human review"
        );
        assert!(
            summary.suggested_action.contains("HUMAN REVIEW REQUIRED"),
            "should require human review, got: {}",
            summary.suggested_action
        );
        // Happy path should have no failure reason codes.
        assert!(
            summary.reason_codes.is_empty(),
            "happy path should have empty reason codes, got: {:?}",
            summary.reason_codes
        );
        // The worktree path should be inside the temp repo.
        assert!(
            summary
                .worktree_path
                .contains("a2a-worktree-integration-task"),
            "worktree path should contain task id, got: {}",
            summary.worktree_path
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn run_worktree_worker_already_in_use_requires_human_review() {
        let tmp = init_test_repo("forge-test-wt-already-in-use");
        let mut existing_lease =
            match crate::agent::a2a::worktree::WorktreeLease::create(&tmp, "busy-task") {
                crate::agent::a2a::worktree::LeaseResult::Ok(lease) => lease,
                other => panic!("expected initial lease, got {other:?}"),
            };

        let adapter: Arc<dyn AiAdapter> = Arc::new(MockAdapter {
            calls: AtomicUsize::new(0),
        });
        let pending_confirms = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let harness = Arc::new(Harness::new_with_pending(
            tmp.clone(),
            pending_confirms.clone(),
        ));
        let emitter: Arc<dyn EventEmitter> =
            Arc::new(AutoApprovePendingEmitter::new(pending_confirms));
        let cancel = Arc::new(Notify::new());

        let raw = ChildAgentRuntime::run_worktree_worker(
            "busy-task",
            "Try to reuse an active worktree",
            adapter,
            harness,
            emitter,
            cancel,
            &tmp,
        )
        .await;

        let summary: crate::agent::a2a::worktree::WorktreeWorkerSummary =
            serde_json::from_str(&raw).expect("should parse WorktreeWorkerSummary");
        assert!(
            summary.needs_human_review,
            "busy worktree should not be reported as a safe automatic outcome"
        );
        assert!(
            summary.suggested_action.contains("HUMAN REVIEW REQUIRED"),
            "should give parent model an explicit review signal, got: {}",
            summary.suggested_action
        );
        assert_eq!(summary.reason_codes, vec!["already_in_use".to_string()]);

        existing_lease.preserve();
        drop(existing_lease);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
