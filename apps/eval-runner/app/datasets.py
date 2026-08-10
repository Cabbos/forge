import re
from pathlib import Path

from app.artifact_hashing import (
    canonical_manifest_digest,
    canonical_tree_manifest,
    tree_digest_or_none,
)
from app.models import EvaluationTask

VERIFIER_ROOT = Path(__file__).resolve().parents[1] / "forgebench" / "verifiers"
VERIFIER_ID_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]*")


def dataset_fingerprint(tasks: list[EvaluationTask]) -> str:
    payload = [_task_fingerprint_payload(task) for task in sorted(tasks, key=lambda item: item.id)]
    return canonical_manifest_digest(payload)


def _task_fingerprint_payload(task: EvaluationTask) -> dict[str, object]:
    task_json = task.model_dump(mode="json")
    return {
        "id": task.id,
        "title": task.title,
        "prompt": task.prompt,
        "fixture": {
            "id": task.fixture_id,
            "version": task.fixture_version,
            "digest": tree_digest_or_none(task.fixture_path),
        },
        "scenario_digest": tree_digest_or_none(task.scenario_path),
        "dataset_split": task_json["dataset_split"],
        "hidden_verifier_id": task.hidden_verifier_id,
        "hidden_verifier_digest": _verifier_digest_or_none(task.hidden_verifier_id),
        "context_files": task.context_files,
        "setup_commands": task.setup_commands,
        "validation_commands": task.validation_commands,
        "post_validation_commands": task.post_validation_commands,
        "pass_to_pass_commands": task.pass_to_pass_commands,
        "fail_to_pass_commands": task.fail_to_pass_commands,
        "verification_command": task.verification_command,
        "full_verification_command": task.full_verification_command,
        "expected_success": task.expected_success,
        "expected_files_changed": task.expected_files_changed,
        "forbidden_files_changed": task.forbidden_files_changed,
        "required_scores": task.required_scores,
        "max_duration_seconds": task.max_duration_seconds,
        "max_model_rounds": task.max_model_rounds,
        "tags": task.tags,
        "metadata": task_json["metadata"],
    }


def _verifier_digest_or_none(verifier_id: str | None) -> str | None:
    if verifier_id is None:
        return None
    if VERIFIER_ID_PATTERN.fullmatch(verifier_id) is None:
        raise ValueError("invalid hidden verifier id")

    required_paths = {"_support.py", f"{verifier_id}.py"}
    entries = [
        entry for entry in canonical_tree_manifest(VERIFIER_ROOT) if entry["path"] in required_paths
    ]
    found_paths = {str(entry["path"]) for entry in entries}
    if found_paths != required_paths:
        raise ValueError(f"unknown hidden verifier: {verifier_id}")
    return canonical_manifest_digest(entries)
