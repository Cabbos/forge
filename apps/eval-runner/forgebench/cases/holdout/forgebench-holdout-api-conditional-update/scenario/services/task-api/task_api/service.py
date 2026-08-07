from collections.abc import Iterable
from typing import Any

from task_api.models import TaskRecord
from task_api.repository import TaskRepository


class RevisionConflictError(RuntimeError):
    pass


class TaskService:
    def __init__(self, repository: TaskRepository) -> None:
        self.repository = repository

    def update(
        self,
        task_id: str,
        changes: dict[str, Any],
        *,
        expected_revision: int | None = None,
    ) -> TaskRecord:
        current = self.repository.get(task_id)
        forbidden = set(changes) - {"title", "status", "priority"}
        if forbidden:
            raise ValueError(f"unsupported task fields: {sorted(forbidden)}")
        updated = current.updated(**changes, revision=current.revision + 1)
        self.repository.save(updated)
        return updated

    def bulk_update(self, updates: Iterable[tuple[str, dict[str, Any]]]) -> list[TaskRecord]:
        snapshot = self.repository.snapshot()
        results: list[TaskRecord] = []
        try:
            for task_id, changes in updates:
                results.append(self.update(task_id, changes))
        except Exception:
            self.repository.restore(snapshot)
            raise
        return results
