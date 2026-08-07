import pytest
from task_api.models import TaskRecord
from task_api.repository import TaskRepository
from task_api.service import RevisionConflictError, TaskService


def test_task_api_validates_and_conditionally_updates() -> None:
    repository = TaskRepository([TaskRecord(id="task-1", title="Ship release")])
    service = TaskService(repository)

    updated = service.update(
        "task-1",
        {"status": "in_progress", "priority": "high"},
        expected_revision=1,
    )

    assert updated.revision == 2
    assert updated.status == "in_progress"
    with pytest.raises(RevisionConflictError):
        service.update("task-1", {"status": "done"}, expected_revision=1)
    with pytest.raises(ValueError, match="title"):
        service.update("task-1", {"title": "  "})


def test_task_api_bulk_update_rolls_back_on_invalid_change() -> None:
    repository = TaskRepository(
        [
            TaskRecord(id="task-1", title="First"),
            TaskRecord(id="task-2", title="Second"),
        ]
    )
    service = TaskService(repository)

    with pytest.raises(ValueError, match="status"):
        service.bulk_update(
            [
                ("task-1", {"status": "in_progress"}),
                ("task-2", {"status": "not-real"}),
            ]
        )

    assert [task.status for task in repository.list()] == ["todo", "todo"]
    assert [task.revision for task in repository.list()] == [1, 1]
