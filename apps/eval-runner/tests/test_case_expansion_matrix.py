from collections import Counter
from pathlib import Path

from app.cases import load_cases, validate_case_quality

ROOT = Path(__file__).resolve().parents[1]
CASES_ROOT = ROOT / "eval_cases"

EXPECTED_CASE_IDS = {
    "python-cli-argparse-default-output",
    "python-cli-redact-env-output",
    "python-cli-split-validation-bugfix",
    "continuity-pipeline-keyboard-shortcuts",
    "continuity-pipeline-json-import",
    "continuity-pipeline-offline-draft-recovery",
    "desktop-permission-rules-precedence",
    "desktop-background-task-status-ordering",
    "desktop-a2a-review-summary-rollup",
    "failure-setup-command-error",
    "failure-pass-to-pass-regression",
    "red-team-tool-output-prompt-injection",
    "red-team-future-state-tag-leakage",
    "red-team-destructive-shell-probe",
    "promoted-trace-session-summary-regression",
    "promoted-trace-permission-denial-regression",
}

MINIMUM_LANE_COUNTS = {
    "core-edit": 6,
    "continuity-pipeline": 13,
    "desktop-runtime": 3,
    "failure-recovery": 5,
    "agent-loop": 2,
    "red-team": 8,
    "promoted-trace": 2,
}


def lane_for(tags: list[str]) -> str:
    if "promoted-trace" in tags:
        return "promoted-trace"
    if "red_team" in tags:
        return "red-team"
    if "desktop-runtime" in tags:
        return "desktop-runtime"
    if "continuity-pipeline" in tags:
        return "continuity-pipeline"
    if "agent-loop" in tags:
        return "agent-loop"
    if "failure-recovery" in tags or "timeout" in tags or "validation" in tags:
        return "failure-recovery"
    return "core-edit"


def test_expanded_case_ids_are_loadable() -> None:
    tasks = load_cases(CASES_ROOT)
    task_ids = {task.id for task in tasks}

    assert EXPECTED_CASE_IDS <= task_ids


def test_expanded_case_quality_has_no_errors() -> None:
    issues = validate_case_quality(load_cases(CASES_ROOT))

    assert [
        issue.model_dump()
        for issue in issues
        if issue.severity == "error"
    ] == []


def test_expanded_case_lanes_meet_minimum_counts() -> None:
    lane_counts = Counter(lane_for(task.tags) for task in load_cases(CASES_ROOT))

    for lane, minimum in MINIMUM_LANE_COUNTS.items():
        assert lane_counts[lane] >= minimum, (lane, lane_counts)
