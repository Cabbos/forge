import json
from pathlib import Path

from pydantic import TypeAdapter, ValidationError

from app.models import EvaluationRun, EvaluationTask


class TaskLoadError(RuntimeError):
    pass


class InMemoryStorage:
    """Simple storage for an MVP API process.

    Runs are intentionally process-local for the first version. The public API and models are shaped
    so the backing store can later move to SQLite/Postgres without changing clients.
    """

    def __init__(self, tasks_path: Path | str) -> None:
        self.tasks_path = Path(tasks_path)
        self._tasks = self._load_tasks()
        self._runs: dict[str, EvaluationRun] = {}

    def _load_tasks(self) -> dict[str, EvaluationTask]:
        if not self.tasks_path.exists():
            return {}

        try:
            raw_tasks = json.loads(self.tasks_path.read_text(encoding="utf-8"))
            tasks = TypeAdapter(list[EvaluationTask]).validate_python(raw_tasks)
        except (OSError, json.JSONDecodeError, ValidationError) as exc:
            raise TaskLoadError(f"Could not load tasks from {self.tasks_path}") from exc

        return {task.id: task for task in tasks}

    def list_tasks(self) -> list[EvaluationTask]:
        return list(self._tasks.values())

    def get_tasks(self, task_ids: list[str] | None) -> list[EvaluationTask]:
        if task_ids is None:
            return self.list_tasks()

        missing = [task_id for task_id in task_ids if task_id not in self._tasks]
        if missing:
            missing_text = ", ".join(missing)
            raise KeyError(f"Unknown task id(s): {missing_text}")

        return [self._tasks[task_id] for task_id in task_ids]

    def save_run(self, run: EvaluationRun) -> None:
        self._runs[run.run_id] = run

    def get_run(self, run_id: str) -> EvaluationRun | None:
        return self._runs.get(run_id)
