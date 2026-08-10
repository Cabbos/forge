from typing import Any

from task_api.repository import TaskRepository
from task_api.service import TaskService


class TaskApi:
    def __init__(self, repository: TaskRepository | None = None) -> None:
        self.service = TaskService(repository or TaskRepository())

    def patch_task(
        self,
        task_id: str,
        payload: dict[str, Any],
        *,
        if_match: int | None = None,
    ) -> dict[str, Any]:
        task = self.service.update(task_id, payload, expected_revision=if_match)
        return {
            "id": task.id,
            "title": task.title,
            "status": task.status,
            "priority": task.priority,
            "revision": task.revision,
        }
