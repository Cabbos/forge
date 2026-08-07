import pytest

from task_api.app import TaskApi
from task_api.models import TaskRecord
from task_api.repository import TaskRepository


def test_api_bulk_edit_rolls_back_a_late_invalid_update() -> None:
    repository = TaskRepository(
        [
            TaskRecord(id="one", title="One"),
            TaskRecord(id="two", title="Two"),
        ]
    )
    api = TaskApi(repository)

    with pytest.raises(ValueError):
        api.patch_tasks(
            [
                {
                    "task_id": "one",
                    "changes": {"status": "in_progress"},
                    "expected_revision": 1,
                },
                {
                    "task_id": "two",
                    "changes": {"status": "invalid"},
                    "expected_revision": 1,
                },
            ]
        )

    assert repository.snapshot() == {
        "one": TaskRecord(id="one", title="One"),
        "two": TaskRecord(id="two", title="Two"),
    }
