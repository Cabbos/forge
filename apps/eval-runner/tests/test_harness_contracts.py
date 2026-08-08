from pathlib import Path

import pytest
from pydantic import ValidationError

from app.harness_contracts import (
    MAX_TOOL_CONTENT_CHARS,
    ContextTier,
    HarnessPolicyV1,
    ProgressKind,
    ProgressSignal,
    StagnationAction,
    TaskContractKind,
    ToolResultEnvelope,
    ToolResultStatus,
    build_context_manifest,
    build_task_execution_contract,
    classify_failure,
    envelope_from_shell_output,
    evaluate_stagnation,
    harness_contract_evidence,
    observed_task_execution_evidence,
    resolve_typed_failure_category,
    task_plan_evidence,
)


def test_tool_result_contract_bounds_large_output_and_keeps_digest() -> None:
    output = "x" * (MAX_TOOL_CONTENT_CHARS + 17)

    envelope = envelope_from_shell_output("read_file", output, "", 0)

    assert envelope.status == ToolResultStatus.TRUNCATED
    assert envelope.truncated is True
    assert len(envelope.content) == MAX_TOOL_CONTENT_CHARS
    assert len(envelope.artifact_digest or "") == 64
    assert envelope.next_evidence == ["retrieve the full shell artifact by digest"]


def test_minimal_harness_policy_is_bounded_and_fingerprintable() -> None:
    policy = HarnessPolicyV1(
        prompt_mode="minimal_v1",
        context_policy="task_local_v1",
        tool_result_policy="bounded_structured_v1",
        stagnation_policy="semantic_progress_v1",
    )
    context = [f"src/file-{index}.py" for index in range(20)]

    selected = policy.selected_context_files(context)
    rendered = policy.render_prompt("Fix the bug.", selected)

    assert len(selected) == 16
    assert rendered.startswith("Outcome: Fix the bug.")
    assert "Execution contract:" in rendered
    assert len(rendered) <= 3_600
    assert len(policy.fingerprint()) == 64


def test_planning_harness_policy_renders_structured_plan_contract() -> None:
    policy = HarnessPolicyV1(
        prompt_mode="planning_v1",
        context_policy="task_local_v1",
        tool_result_policy="bounded_structured_v1",
        stagnation_policy="semantic_progress_v1",
    )

    rendered = policy.render_prompt("Fix the cross-file bug.", ["src/a.py", "src/b.py"])

    assert rendered.startswith("Outcome: Fix the cross-file bug.")
    assert "Plan-first contract:" in rendered
    assert "task_plan" in rendered
    assert len(rendered) <= 3_600


def test_truncated_tool_result_without_artifact_digest_is_rejected() -> None:
    with pytest.raises(ValidationError, match="artifact_digest"):
        ToolResultEnvelope(
            tool="read_file",
            status=ToolResultStatus.TRUNCATED,
            summary="truncated",
            truncated=True,
        )


def test_context_manifest_is_minimal_and_detects_duplicates_and_conflicts(
    tmp_path: Path,
) -> None:
    (tmp_path / "a").mkdir()
    (tmp_path / "b").mkdir()
    (tmp_path / "a" / "same.txt").write_text("same\n", encoding="utf-8")
    (tmp_path / "b" / "same.txt").write_text("same\n", encoding="utf-8")
    (tmp_path / "a" / "diff.txt").write_text("left\n", encoding="utf-8")
    (tmp_path / "b" / "diff.txt").write_text("right\n", encoding="utf-8")

    manifest = build_context_manifest(
        ["a/same.txt", "b/same.txt", "a/diff.txt", "b/diff.txt"], tmp_path
    )

    assert manifest.mode == "minimal"
    assert manifest.total_bytes == len("same\n") * 2 + len("left\n") + len("right\n")
    assert manifest.total_token_estimate > 0
    assert manifest.duplicate_groups == [["a/same.txt", "b/same.txt"]]
    assert manifest.conflict_groups == [["a/diff.txt", "b/diff.txt"]]
    assert all(item.tier == ContextTier.TASK_LOCAL for item in manifest.items)
    assert all("content" not in item.model_dump() for item in manifest.items)


def test_stagnation_redirects_once_then_stops() -> None:
    signals = [
        ProgressSignal(round=1, kind=ProgressKind.NO_PROGRESS, evidence="same state"),
        ProgressSignal(round=2, kind=ProgressKind.NO_PROGRESS, evidence="same state"),
    ]

    redirect = evaluate_stagnation(signals)
    stopped = evaluate_stagnation(signals, redirects_used=1)

    assert redirect.action == StagnationAction.REDIRECT
    assert redirect.redirects_used == 1
    assert redirect.recovery_instruction is not None
    assert stopped.action == StagnationAction.STOP
    assert stopped.redirects_used == 1


@pytest.mark.parametrize(
    ("error", "legacy_category", "verification_passed", "expected"),
    [
        (None, "none", True, "passed"),
        ("api returned 503", "runner_error", None, "provider_error"),
        ("unexpected trace shape", "runner_error", None, "harness_contract_error"),
        (None, "scope_violation", True, "scope_violation"),
    ],
)
def test_failure_taxonomy_never_emits_generic_runner_error(
    error: str | None,
    legacy_category: str,
    verification_passed: bool | None,
    expected: str,
) -> None:
    evidence = classify_failure(
        error=error,
        legacy_category=legacy_category,
        verification_passed=verification_passed,
    )

    assert evidence.primary.value == expected
    assert evidence.primary.value != "runner_error"


def test_harness_contract_evidence_marks_missing_progress_explicitly(tmp_path: Path) -> None:
    evidence = harness_contract_evidence(
        task_context_files=[],
        workspace=tmp_path,
        tool_calls=[("pytest", "passed", "", 0)],
    )

    assert evidence["event_type"] == "harness_contract_evidence"
    assert evidence["tool_results"]["source"] == "derived_shell_output"
    assert evidence["progress"]["source"] == "missing"
    assert evidence["progress"]["stagnation"]["action"] == "continue"
    assert evidence["failure_taxonomy"]["primary"] == "model_incomplete"


def test_typed_failure_category_preserves_legacy_artifacts_and_rejects_bad_taxonomy() -> None:
    assert resolve_typed_failure_category([], "none") == "none"
    assert resolve_typed_failure_category(
        [{"event_type": "harness_contract_evidence", "failure_taxonomy": {"primary": "passed"}}],
        "none",
    ) == "passed"
    assert resolve_typed_failure_category(
        [{"event_type": "harness_contract_evidence", "failure_taxonomy": {"primary": "nope"}}],
        "none",
    ) == "harness_contract_error"


def test_task_execution_contract_requires_all_multi_file_inspection_and_verification() -> None:
    contract = build_task_execution_contract(
        expected_files_changed=["src/a.py", "src/b.py"],
        forbidden_files_changed=["package.json"],
        context_files=[],
        validation_commands=["pytest tests/test_ab.py"],
        verification_command=None,
        tags=["multi-file"],
    )

    assert contract.kind == TaskContractKind.MULTI_FILE
    evidence = task_plan_evidence(
        contract,
        {
            "goal": "preserve the cross-file contract",
            "steps": [
                {"id": "inspect", "action": "inspect", "files": ["src/a.py", "src/b.py"]},
                {"id": "edit", "action": "edit", "files": ["src/a.py", "src/b.py"]},
                {"id": "verify", "action": "verify", "files": []},
            ],
        },
    )

    assert evidence["status"] == "valid"
    assert evidence["inspected_files"] == ["src/a.py", "src/b.py"]


def test_task_plan_evidence_rejects_out_of_scope_edit_and_missing_verification() -> None:
    contract = build_task_execution_contract(
        expected_files_changed=["src/a.py"],
        forbidden_files_changed=["package.json"],
        context_files=[],
        validation_commands=["pytest"],
        verification_command=None,
        tags=[],
    )

    with pytest.raises(ValueError, match="task_plan_edit_out_of_scope"):
        task_plan_evidence(
            contract,
            {
                "goal": "break scope",
                "steps": [
                    {"id": "inspect", "action": "inspect", "files": ["src/a.py"]},
                    {"id": "edit", "action": "edit", "files": ["package.json"]},
                    {"id": "verify", "action": "verify", "files": []},
                ],
            },
        )

    with pytest.raises(ValueError, match="task_plan_missing_verification"):
        task_plan_evidence(
            contract,
            {
                "goal": "forget validation",
                "steps": [{"id": "inspect", "action": "inspect", "files": ["src/a.py"]}],
            },
        )


def test_observed_task_execution_evidence_reports_phase_coverage() -> None:
    contract = build_task_execution_contract(
        expected_files_changed=["src/a.py", "src/b.py"],
        forbidden_files_changed=[],
        context_files=[],
        validation_commands=["pytest tests/test_ab.py"],
        verification_command=None,
        tags=["multi-file"],
    )

    observed = observed_task_execution_evidence(
        contract,
        tool_calls=[
            ("read_file src/a.py", "", "", 0),
            ("read_file src/b.py", "", "", 0),
            ("pytest tests/test_ab.py", "ok", "", 0),
        ],
        changed_files=["src/a.py", "src/b.py"],
        verification_passed=True,
    )

    assert observed["status"] == "complete"
    assert observed["missing_inspection_files"] == []
    assert observed["unexpected_changed_files"] == []
