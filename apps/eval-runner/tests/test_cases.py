import json
from datetime import UTC, datetime
from pathlib import Path

import pytest

from app.cases import CaseLoadError, load_cases
from app.models import AgentTrace, FailureCategory, ForgeRunEvidence, VerificationResult


def write_case(cases_dir: Path, case_id: str, *, title: str | None = None) -> Path:
    case_dir = cases_dir / case_id
    fixture = case_dir / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")
    (case_dir / "case.json").write_text(
        json.dumps(
            {
                "task": {
                    "id": case_id,
                    "title": title or case_id,
                    "prompt": f"Complete {case_id}.",
                    "fixture_path": "fixture",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                    "expected_files_changed": ["src/app.py"],
                }
            }
        ),
        encoding="utf-8",
    )
    return case_dir


def make_trace(
    task_id: str,
    *,
    error: str | None = None,
    failure_reason: str | None = None,
) -> AgentTrace:
    now = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    failed = error is not None or failure_reason is not None
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Fix production issue {task_id}.",
        model="local-forge",
        provider="forge",
        context_files=["src/app.py"],
        changed_files=["src/app.py"],
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[".env"],
        final_answer="failed" if failed else "done",
        verification_result=VerificationResult(
            command="pytest",
            passed=not failed,
            stdout="" if failed else "passed",
            stderr="failed" if failed else "",
            exit_code=1 if failed else 0,
            duration_ms=120,
        ),
        error=error,
        failure_reason=failure_reason,
        failure_category=FailureCategory.VERIFICATION_FAILED if failed else FailureCategory.NONE,
        started_at=now,
        ended_at=now,
        duration_ms=120,
    )


def test_failed_trace_can_be_promoted_to_eval_case() -> None:
    from app.trace_import import case_from_trace

    trace = make_trace(
        task_id="real-user-failure",
        error="verification_failed",
        failure_reason="test failed",
    )
    task = case_from_trace(trace)

    assert task.id == "real-user-failure"
    assert task.title == "Promoted trace: real-user-failure"
    assert task.prompt == "Fix production issue real-user-failure."
    assert task.expected_success is False
    assert task.expected_files_changed == ["src/app.py"]
    assert task.forbidden_files_changed == [".env"]
    assert task.verification_command == "pytest"
    assert task.metadata["source"] == "trace"
    assert task.metadata["failure_reason"] == "test failed"


def test_trace_promotion_includes_forge_run_evidence_metadata() -> None:
    from app.trace_import import case_from_trace

    trace = make_trace(
        task_id="forge-runtime-failure",
        error="verification_failed",
        failure_reason="test failed",
    ).model_copy(
        update={
            "forge_run_evidence": ForgeRunEvidence(
                session_id="session-1",
                loop_task_id="loop-1",
                prompt="Fix the runtime issue.",
                normalized_goal="fix-runtime",
                failure_category="verification_failed",
                verification={"command": "pytest", "passed": False},
            )
        }
    )

    task = case_from_trace(trace)

    assert task.metadata["normalized_goal"] == "fix-runtime"
    assert task.metadata["failure_category"] == "verification_failed"
    assert task.metadata["forge_run_evidence"]["session_id"] == "session-1"
    assert task.metadata["forge_run_evidence"]["loop_task_id"] == "loop-1"


def test_trace_promotion_preserves_v1_evidence_with_unknown_v2_fields() -> None:
    from app.trace_import import case_from_trace

    trace = make_trace(task_id="legacy-forge-runtime").model_copy(
        update={
            "forge_run_evidence": ForgeRunEvidence(
                schema_version=1,
                session_id="session-legacy",
                prompt="Legacy Forge trace.",
            )
        }
    )

    task = case_from_trace(trace)

    assert task.metadata["forge_run_evidence"]["schema_version"] == 1
    assert task.metadata["forge_run_evidence"]["completion_eligibility"] == {"status": "unknown"}


def test_load_traces_normalizes_single_desktop_trace_payload(tmp_path: Path) -> None:
    from app.trace_import import load_traces

    trace_path = tmp_path / "desktop-trace.json"
    trace_path.write_text(
        json.dumps(
            {
                "task_id": "desktop-session-1",
                "session_id": "session-1",
                "user_prompt": "Fix the button feedback.",
                "provider": "forge",
                "model": "local-forge",
                "loop_task": {
                    "task_id": "loop-1",
                    "recovery_state": {"kind": "orphaned", "notice": "Recovered stale lease."},
                    "completion_result": {
                        "status": "blocked",
                        "commit_eligible": False,
                        "commit_blockers": ["missing_human_review"],
                        "eligibility_facts": {
                            "review": {
                                "status": "missing",
                                "reason": "review_decision_missing",
                            }
                        },
                    },
                },
                "tool_calls": [],
                "shell_outputs": [],
                "file_diffs": [],
                "changed_files": ["src/App.tsx"],
                "verification_result": {
                    "command": "npm test",
                    "passed": True,
                    "stdout": "passed",
                    "stderr": "",
                    "exit_code": 0,
                    "duration_ms": 50,
                },
                "final_answer": "Done.",
                "input_tokens": 12,
                "output_tokens": 4,
                "failure_category": "orphaned",
                "duration_ms": 100,
                "a2a_child_capsules": [
                    {
                        "child_task_id": "child-1",
                        "status": "completed",
                        "artifact_titles": ["Patch proposal"],
                        "changed_files": ["src/App.tsx"],
                        "review_decision": "approved",
                        "review_gate": {"kind": "approved"},
                        "next_action": "Review child evidence.",
                    }
                ],
                "compact_count": 0,
                "headless_continuity_formed_count": 1,
            }
        ),
        encoding="utf-8",
    )

    traces = load_traces(trace_path)

    assert len(traces) == 1
    trace = traces[0]
    assert trace.task_id == "desktop-session-1"
    assert trace.failure_category == FailureCategory.RUNNER_ERROR
    assert trace.forge_run_evidence is not None
    assert trace.forge_run_evidence.session_id == "session-1"
    assert trace.forge_run_evidence.loop_task_id == "loop-1"
    assert trace.forge_run_evidence.schema_version == 2
    assert trace.forge_run_evidence.completion_eligibility == {
        "status": "blocked",
        "commit_eligible": False,
        "commit_blockers": ["missing_human_review"],
        "facts": {
            "review": {
                "status": "missing",
                "reason": "review_decision_missing",
            }
        },
    }
    assert trace.forge_run_evidence.failure_category == "orphaned"
    assert trace.forge_run_evidence.recovery == {
        "kind": "orphaned",
        "notice": "Recovered stale lease.",
    }
    assert trace.forge_run_evidence.a2a_child_capsules[0]["child_task_id"] == "child-1"
    assert trace.forge_run_evidence.a2a_child_capsules[0]["review_gate"]["kind"] == "approved"


def test_load_traces_keeps_legacy_trace_without_forge_evidence(tmp_path: Path) -> None:
    from app.trace_import import load_traces

    trace_path = tmp_path / "legacy-traces.json"
    trace_path.write_text(
        json.dumps({"traces": [make_trace("legacy-ok").model_dump(mode="json")]}),
        encoding="utf-8",
    )

    traces = load_traces(trace_path)

    assert traces[0].forge_run_evidence is None


def test_load_cases_reads_case_directories_and_resolves_fixture_paths(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    first_dir = write_case(cases_dir, "small-edit-success", title="Small edit succeeds")
    write_case(cases_dir, "validation-failure", title="Validation fails")

    tasks = load_cases(cases_dir)

    assert [task.id for task in tasks] == ["small-edit-success", "validation-failure"]
    assert tasks[0].title == "Small edit succeeds"
    assert tasks[0].fixture_path is not None
    assert Path(tasks[0].fixture_path).is_absolute()
    assert Path(tasks[0].fixture_path) == first_dir / "fixture"


def test_load_cases_rejects_duplicate_task_ids(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(cases_dir, "duplicate")
    second = write_case(cases_dir, "duplicate-copy")
    payload = json.loads((second / "case.json").read_text(encoding="utf-8"))
    payload["task"]["id"] = "duplicate"
    (second / "case.json").write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CaseLoadError, match="Duplicate task id"):
        load_cases(cases_dir)


def test_load_cases_reads_a_json_file_with_multiple_tasks(tmp_path: Path) -> None:
    cases_file = tmp_path / "cases.json"
    cases_file.write_text(
        json.dumps(
            [
                {
                    "id": "small-edit-success",
                    "title": "Small edit succeeds",
                    "prompt": "Make a safe edit.",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                },
                {
                    "task": {
                        "id": "validation-failure",
                        "title": "Validation fails",
                        "prompt": "Trigger a validation failure.",
                        "context_files": ["src/app.py"],
                        "verification_command": "pytest",
                        "expected_success": False,
                    }
                },
            ]
        ),
        encoding="utf-8",
    )

    tasks = load_cases(cases_file)

    assert [task.id for task in tasks] == ["small-edit-success", "validation-failure"]
    assert tasks[1].expected_success is False


def test_load_cases_includes_real_forge_session_backtests() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected_ids = {
        "forge-session-normalize-input",
        "forge-session-date-utils",
        "forge-session-truncate-text",
        "forge-session-capitalize",
        "forge-session-kebab-case",
    }

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert task.fixture_path is not None
        assert Path(task.fixture_path).exists()
        assert task.setup_commands == ["npm install"]
        assert task.validation_commands
        assert "real-forge-session" in task.tags


def test_load_cases_includes_agent_loop_stop_reason_backtests() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected = {
        "agent-loop-tool-loop-detected": "tool_loop_detected",
        "agent-loop-repeated-no-progress": "repeated_no_progress",
    }

    assert expected.keys() <= task_by_id.keys()
    for task_id, stop_reason in expected.items():
        task = task_by_id[task_id]
        mock = task.metadata["mock"]
        raw_stop_reasons = {
            event.get("stop_reason") or (event.get("state") or {}).get("stop_reason")
            for event in mock["raw_events"]
        }

        assert task.expected_success is False
        assert "agent-loop" in task.tags
        assert mock["failure_category"] == "budget_exhausted"
        assert mock["error"] == stop_reason
        assert stop_reason in raw_stop_reasons


def test_load_cases_includes_gateway_eval_pack() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected_ids = {
        "gateway-local-parity-dry-run",
        "gateway-degraded-fallback",
        "gateway-read-only-owner-diagnostics",
        "gateway-patch-proposal-owner-gate",
        "gateway-direct-write-owner-blocked",
        "gateway-lease-timeout-recovery",
        "gateway-duplicate-input-prevention",
    }

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert "gateway" in task.tags
        assert "forge-runtime-evidence" in task.tags
        assert task.metadata.get("contract_only") is True
        evidence = task.metadata["mock"]["forge_run_evidence"]
        assert evidence["gateway"]
        assert evidence["verification"]["passed"] is True


def test_load_cases_includes_a2a_eval_pack() -> None:
    from app.runner import DeterministicMockRunner
    from app.scoring import score_trace

    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    passing_ids = {
        "a2a-child-completed-review-evidence",
        "a2a-child-context-capsule-contract",
        "a2a-child-failed-recovery-evidence",
        "a2a-child-runtime-event-file-facts",
        "a2a-child-review-gate-identity",
        "a2a-child-worktree-facts",
    }
    failing_ids = {
        "a2a-child-failure-recovery-policy",
        "a2a-child-runtime-event-missing-file-fact",
        "a2a-child-review-gate-blocking-states",
        "a2a-child-worktree-missing-facts",
    }
    expected_ids = passing_ids | failing_ids

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert "a2a" in task.tags
        assert "a2a-eval-pack" in task.tags
        assert "forge-runtime-evidence" in task.tags
        assert task.metadata.get("contract_only") is True
        evidence = task.metadata["mock"]["forge_run_evidence"]
        assert evidence["a2a_child_capsules"]
        assert evidence["verification"]["passed"] is (task_id in passing_ids)
        prepared_context_json = json.dumps(evidence["prepared_context"])
        for capsule in evidence["a2a_child_capsules"]:
            assert capsule["capsule_id"]
            assert capsule["child_goal"]
            assert capsule["estimated_tokens"] > 0
            assert capsule["capsule_id"] in prepared_context_json

    blocking_trace = DeterministicMockRunner().run_task(
        task_by_id["a2a-child-review-gate-blocking-states"]
    )
    blocking_score = score_trace(blocking_trace)["forge_a2a_child_evidence_complete_ok"]
    assert blocking_score.score == 0.0
    assert "review_gate_stale_review" in (blocking_score.explanation or "")
    assert "review_gate_wrong_parent" in (blocking_score.explanation or "")
    assert "review_gate_missing_evidence" in (blocking_score.explanation or "")

    recovery_trace = DeterministicMockRunner().run_task(
        task_by_id["a2a-child-failure-recovery-policy"]
    )
    recovery_score = score_trace(recovery_trace)["forge_a2a_child_evidence_complete_ok"]
    assert recovery_score.score == 0.0
    recovery_explanation = recovery_score.explanation or ""
    assert "recovery_action_requires_human_approval:retry" in recovery_explanation
    assert "retry_missing_new_attempt" in recovery_explanation
    assert "retry_missing_new_lease" in recovery_explanation
    assert "recovery_action_auto_executes:abandon" in recovery_explanation

    runtime_trace = DeterministicMockRunner().run_task(
        task_by_id["a2a-child-runtime-event-file-facts"]
    )
    runtime_score = score_trace(runtime_trace)["forge_a2a_child_evidence_complete_ok"]
    assert runtime_score.score == 1.0

    missing_runtime_trace = DeterministicMockRunner().run_task(
        task_by_id["a2a-child-runtime-event-missing-file-fact"]
    )
    missing_runtime_score = score_trace(missing_runtime_trace)[
        "forge_a2a_child_evidence_complete_ok"
    ]
    assert missing_runtime_score.score == 0.0
    missing_runtime_explanation = missing_runtime_score.explanation or ""
    assert "runtime_event_missing:file_fact" in missing_runtime_explanation

    worktree_trace = DeterministicMockRunner().run_task(
        task_by_id["a2a-child-worktree-missing-facts"]
    )
    worktree_score = score_trace(worktree_trace)["forge_a2a_child_evidence_complete_ok"]
    assert worktree_score.score == 0.0
    worktree_explanation = worktree_score.explanation or ""
    assert "worktree_missing_path" in worktree_explanation
    assert "worktree_missing_test_report" in worktree_explanation
    assert "worktree_missing_cleanup_status" in worktree_explanation


def test_load_cases_includes_runtime_recovery_eval_pack() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected_ids = {
        "runtime-recovery-orphaned-run",
        "runtime-recovery-interrupted-shell",
        "runtime-recovery-pending-confirmation",
        "runtime-recovery-provider-usage-unknown",
        "runtime-recovery-verification-missing",
    }

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert "runtime-recovery" in task.tags
        assert "forge-runtime-evidence" in task.tags
        assert task.metadata.get("contract_only") is True
        evidence = task.metadata["mock"]["forge_run_evidence"]
        assert evidence["recovery_cases"]
        assert evidence["verification"]["passed"] is True


def test_load_cases_includes_memory_eval_pack() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected_ids = {
        "memory-recall-correct",
        "memory-recall-duplicate-overbudget",
        "memory-recall-continuity-dedupe",
        "memory-recall-wrong-project",
        "memory-recall-wrong-profile",
        "memory-recall-archived-forgotten",
        "memory-recall-hidden-body-leak",
    }

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert "memory-eval-pack" in task.tags
        assert "forge-runtime-evidence" in task.tags
        assert task.metadata.get("contract_only") is True
        evidence = task.metadata["mock"]["forge_run_evidence"]
        assert evidence["memory_audit"]
        assert evidence["verification"]["passed"] is True


def test_case_quality_reports_missing_verification_for_executable_case(tmp_path: Path) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "needs-verification",
                "title": "Needs verification",
                "prompt": "Change src/foo.py",
                "context_files": ["src/foo.py"],
                "expected_files_changed": ["src/foo.py"],
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "needs-verification",
            "severity": "warning",
            "code": "missing_verification",
            "message": "Executable eval case has no verification_command or validation_commands.",
        }
    ]


def test_case_quality_reports_missing_expected_files_for_executable_case(
    tmp_path: Path,
) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "missing-expected-files",
                "title": "Missing expected files",
                "prompt": "Change src/foo.py",
                "verification_command": "pytest",
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "missing-expected-files",
            "severity": "warning",
            "code": "missing_expected_files",
            "message": "Executable eval case has no expected_files_changed assertions.",
        }
    ]


def test_case_quality_reports_missing_fixture_path(tmp_path: Path) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "missing-fixture",
                "title": "Missing fixture",
                "prompt": "Change src/foo.py",
                "fixture_path": "does-not-exist",
                "verification_command": "pytest",
                "expected_files_changed": ["src/foo.py"],
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "missing-fixture",
            "severity": "error",
            "code": "missing_fixture_path",
            "message": "Eval case fixture_path does not exist.",
        }
    ]


def test_case_quality_allows_contract_only_cases_without_executable_assertions(
    tmp_path: Path,
) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "contract-only",
                "title": "Contract only",
                "prompt": "Check contract shape.",
                "metadata": {"contract_only": True},
            }
        ),
        encoding="utf-8",
    )

    assert validate_case_quality(load_cases(case)) == []


def test_prompt_mutation_creates_stable_user_style_variant() -> None:
    from app.models import EvaluationTask
    from app.prompt_mutation import mutate_prompt

    task = EvaluationTask(
        id="a",
        title="A",
        prompt="Implement normalizeInput.",
        context_files=["src/normalize.ts"],
    )

    variant = mutate_prompt(task, style="terse-bug-report")

    assert variant.id == "a__terse-bug-report"
    assert "normalizeInput" in variant.prompt
    assert variant.context_files == ["src/normalize.ts"]
    assert variant.metadata["base_task_id"] == "a"
    assert variant.metadata["mutation_style"] == "terse-bug-report"


def test_red_team_case_marks_secret_leak_probe() -> None:
    from app.models import EvaluationTask
    from app.red_team import secret_leak_probe

    task = EvaluationTask(id="base", title="Base", prompt="Summarize the repo.")
    probe = secret_leak_probe(task)

    assert probe.id == "base__red-team-secret-leak"
    assert "red_team" in probe.tags
    assert "secret_leak" in probe.tags
    assert probe.metadata["base_task_id"] == "base"
    assert probe.metadata["red_team_category"] == "secret_leak"
