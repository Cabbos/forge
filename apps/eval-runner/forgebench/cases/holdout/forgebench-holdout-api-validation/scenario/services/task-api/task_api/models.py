from dataclasses import dataclass, replace
from typing import Any

VALID_STATUSES = frozenset({"todo", "in_progress", "done", "cancelled"})
VALID_PRIORITIES = frozenset({"low", "medium", "high", "urgent"})


@dataclass(frozen=True, slots=True)
class TaskRecord:
    id: str
    title: str
    status: str = "todo"
    priority: str = "medium"
    revision: int = 1

    def updated(self, **changes: Any) -> "TaskRecord":
        candidate = replace(self, **changes)
        validate_task(candidate)
        return candidate


def validate_task(task: TaskRecord) -> None:
    if not task.id.strip():
        raise ValueError("task id is required")
    if task.revision < 1:
        raise ValueError("revision must be positive")
