import hashlib
import json

from app.models import EvaluationTask


def dataset_fingerprint(tasks: list[EvaluationTask]) -> str:
    payload = [
        {
            "id": task.id,
            "prompt": task.prompt,
            "context_files": task.context_files,
            "fixture_path": task.fixture_path,
            "setup_commands": task.setup_commands,
            "validation_commands": task.validation_commands,
            "post_validation_commands": task.post_validation_commands,
            "verification_command": task.verification_command,
            "expected_files_changed": task.expected_files_changed,
            "forbidden_files_changed": task.forbidden_files_changed,
            "tags": task.tags,
            "metadata": task.metadata,
        }
        for task in sorted(tasks, key=lambda item: item.id)
    ]
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()
