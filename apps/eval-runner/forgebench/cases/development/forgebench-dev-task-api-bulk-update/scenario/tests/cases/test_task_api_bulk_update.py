import pytest
from task_api.models import TaskRecord
from task_api.repository import TaskRepository
from task_api.service import TaskService


def test_mixed_bulk_update_rolls_back() -> None:
    repository = TaskRepository(
        [TaskRecord(id="one", title="One"), TaskRecord(id="two", title="Two")]
    )
    with pytest.raises(ValueError):
        TaskService(repository).bulk_update(
            [("one", {"status": "in_progress"}), ("two", {"status": "invalid"})]
        )
    assert [(task.status, task.revision) for task in repository.list()] == [
        ("todo", 1),
        ("todo", 1),
    ]
