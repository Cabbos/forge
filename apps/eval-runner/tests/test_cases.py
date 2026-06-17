import json
from pathlib import Path

import pytest

from app.cases import CaseLoadError, load_cases


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
            event.get("stop_reason")
            or (event.get("state") or {}).get("stop_reason")
            for event in mock["raw_events"]
        }

        assert task.expected_success is False
        assert "agent-loop" in task.tags
        assert mock["failure_category"] == "budget_exhausted"
        assert mock["error"] == stop_reason
        assert stop_reason in raw_stop_reasons


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
