import json
from pathlib import Path
from typing import Any

from pydantic import TypeAdapter, ValidationError

from app.models import CaseQualityIssue, DatasetSplit, EvaluationTask
from app.prompt_mutation import mutate_prompt
from app.scoring import KNOWN_SCORE_NAMES

CASE_FILE_NAMES = {"case.json", "task.json"}


class CaseLoadError(RuntimeError):
    pass


def load_cases(path: Path | str) -> list[EvaluationTask]:
    """Load eval case task definitions from a JSON file or case directory."""

    case_path = Path(path)
    if not case_path.exists():
        return []

    try:
        tasks = (
            _load_case_directory(case_path)
            if case_path.is_dir()
            else _load_case_file(case_path, case_path.parent)
        )
    except (OSError, json.JSONDecodeError, ValidationError, TypeError, ValueError) as exc:
        raise CaseLoadError(f"Could not load eval cases from {case_path}") from exc

    _raise_for_duplicate_ids(tasks)
    return tasks


def validate_case_quality(tasks: list[EvaluationTask]) -> list[CaseQualityIssue]:
    issues: list[CaseQualityIssue] = []
    for task in tasks:
        contract_only = bool(task.metadata.get("contract_only"))
        real_coding = task.metadata.get("real_coding") is True
        expected_no_change = task.metadata.get("expected_no_change") is True
        has_verification = bool(task.verification_command or task.validation_commands)

        if not contract_only and not has_verification:
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="warning",
                    code="missing_verification",
                    message=(
                        "Executable eval case has no verification_command or validation_commands."
                    ),
                )
            )
        if not contract_only and not expected_no_change and not task.expected_files_changed:
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="warning",
                    code="missing_expected_files",
                    message="Executable eval case has no expected_files_changed assertions.",
                )
            )
        if expected_no_change and task.expected_files_changed:
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="error",
                    code="expected_no_change_conflict",
                    message="No-change eval case cannot declare expected_files_changed.",
                )
            )
        if task.fixture_path and not Path(task.fixture_path).exists():
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="error",
                    code="missing_fixture_path",
                    message="Eval case fixture_path does not exist.",
                )
            )
        if task.scenario_path and not Path(task.scenario_path).exists():
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="error",
                    code="missing_scenario_path",
                    message="Eval case scenario_path does not exist.",
                )
            )
        if real_coding:
            required_fields = (
                ("fixture_id", task.fixture_id, "missing_fixture_id"),
                ("fixture_version", task.fixture_version, "missing_fixture_version"),
                ("fixture_path", task.fixture_path, "missing_fixture_path"),
                ("dataset_split", task.dataset_split, "missing_dataset_split"),
                ("hidden_verifier_id", task.hidden_verifier_id, "missing_hidden_verifier"),
                (
                    "full_verification_command",
                    task.full_verification_command,
                    "missing_full_verification",
                ),
            )
            for field_name, value, code in required_fields:
                if value is None or (isinstance(value, str) and not value.strip()):
                    issues.append(
                        CaseQualityIssue(
                            task_id=task.id,
                            severity="error",
                            code=code,
                            message=f"Real coding eval case requires {field_name}.",
                        )
                    )
            if (
                task.fixture_id is not None
                and task.fixture_version is not None
                and (task.fixture_id, task.fixture_version) != ("forgebench-monorepo", "1.0.0")
            ):
                issues.append(
                    CaseQualityIssue(
                        task_id=task.id,
                        severity="error",
                        code="non_canonical_real_coding_fixture",
                        message=(
                            "Real coding eval cases must use "
                            "forgebench-monorepo fixture version 1.0.0."
                        ),
                    )
                )
        if (
            task.dataset_split == DatasetSplit.HOLDOUT
            and task.metadata.get("holdout_unsealed") is True
        ):
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="error",
                    code="holdout_unsealed",
                    message="Holdout eval case must remain sealed.",
                )
            )
        for score_name in sorted(set(task.required_scores) - KNOWN_SCORE_NAMES):
            issues.append(
                CaseQualityIssue(
                    task_id=task.id,
                    severity="error",
                    code="unknown_required_score",
                    message=f"Eval case declares unknown required score: {score_name}",
                )
            )
    return issues


def expand_prompt_mutations(
    tasks: list[EvaluationTask],
    *,
    styles: list[str],
    mutations_only: bool = False,
) -> list[EvaluationTask]:
    if not styles:
        return tasks
    mutated = [mutate_prompt(task, style=style) for task in tasks for style in styles]
    return mutated if mutations_only else [*tasks, *mutated]


def _load_case_directory(path: Path) -> list[EvaluationTask]:
    case_files = sorted(file for file in path.rglob("*.json") if file.name in CASE_FILE_NAMES)
    tasks: list[EvaluationTask] = []
    for case_file in case_files:
        tasks.extend(_load_case_file(case_file, case_file.parent))
    return tasks


def _load_case_file(path: Path, base_dir: Path) -> list[EvaluationTask]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    raw_tasks = _extract_task_payloads(payload)
    prepared_tasks = [_resolve_case_paths(raw_task, base_dir) for raw_task in raw_tasks]
    return TypeAdapter(list[EvaluationTask]).validate_python(prepared_tasks)


def _extract_task_payloads(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [_extract_single_task(item) for item in payload]
    if isinstance(payload, dict) and "tasks" in payload:
        tasks = payload["tasks"]
        if not isinstance(tasks, list):
            raise ValueError("tasks must be a list")
        return [_extract_single_task(item) for item in tasks]
    return [_extract_single_task(payload)]


def _extract_single_task(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValueError("case entries must be JSON objects")
    task = payload.get("task", payload)
    if not isinstance(task, dict):
        raise ValueError("case task must be a JSON object")
    return dict(task)


def _resolve_case_paths(raw_task: dict[str, Any], base_dir: Path) -> dict[str, Any]:
    resolved_task = dict(raw_task)
    for field_name in ("fixture_path", "scenario_path"):
        raw_path = raw_task.get(field_name)
        if raw_path is None:
            continue
        path = Path(str(raw_path)).expanduser()
        if not path.is_absolute():
            path = base_dir / path
        resolved_task[field_name] = str(path.resolve())
    return resolved_task


def _raise_for_duplicate_ids(tasks: list[EvaluationTask]) -> None:
    seen: set[str] = set()
    duplicates: list[str] = []
    for task in tasks:
        if task.id in seen:
            duplicates.append(task.id)
        seen.add(task.id)
    if duplicates:
        duplicate_text = ", ".join(sorted(set(duplicates)))
        raise CaseLoadError(f"Duplicate task id(s): {duplicate_text}")
