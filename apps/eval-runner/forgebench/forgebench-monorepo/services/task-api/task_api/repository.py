from collections.abc import Iterable

from task_api.models import TaskRecord, validate_task


class TaskRepository:
    def __init__(self, tasks: Iterable[TaskRecord] = ()) -> None:
        self._tasks = {task.id: task for task in tasks}

    def get(self, task_id: str) -> TaskRecord:
        try:
            return self._tasks[task_id]
        except KeyError as exc:
            raise KeyError(f"unknown task: {task_id}") from exc

    def list(self) -> list[TaskRecord]:
        return list(self._tasks.values())

    def save(self, task: TaskRecord) -> None:
        validate_task(task)
        self._tasks[task.id] = task

    def snapshot(self) -> dict[str, TaskRecord]:
        return dict(self._tasks)

    def restore(self, snapshot: dict[str, TaskRecord]) -> None:
        self._tasks = dict(snapshot)
