from pathlib import Path

import pytest
from pydantic import ValidationError

from app.harness_contracts import (
    MAX_TOOL_CONTENT_CHARS,
    ContextTier,
    ProgressKind,
    ProgressSignal,
    StagnationAction,
    ToolResultEnvelope,
    ToolResultStatus,
    build_context_manifest,
    classify_failure,
    envelope_from_shell_output,
    evaluate_stagnation,
    harness_contract_evidence,
)


def test_tool_result_contract_bounds_large_output_and_keeps_digest() -> None:
    output = "x" * (MAX_TOOL_CONTENT_CHARS + 17)

    envelope = envelope_from_shell_output("read_file", output, "", 0)

    assert envelope.status == ToolResultStatus.TRUNCATED
    assert envelope.truncated is True
    assert len(envelope.content) == MAX_TOOL_CONTENT_CHARS
    assert len(envelope.artifact_digest or "") == 64
    assert envelope.next_evidence == ["retrieve the full shell artifact by digest"]


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
