from pathlib import Path
from typing import Any

from _support import run_python_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import os
import sys
from pathlib import Path
import pytest

root = Path(os.environ["FORGEBENCH_WORKSPACE"])
sys.path.insert(0, str(root / "services" / "task-api"))
from task_api.models import TaskRecord
from task_api.repository import TaskRepository
from task_api.service import TaskService

def test_bulk_update_restores_every_record_after_late_failure():
    repository = TaskRepository([
        TaskRecord(id="one", title="One", revision=3),
        TaskRecord(id="two", title="Two", revision=5),
        TaskRecord(id="three", title="Three", revision=8),
    ])
    before = repository.snapshot()
    with pytest.raises(ValueError):
        TaskService(repository).bulk_update([
            ("one", {"priority": "urgent"}),
            ("two", {"title": "Renamed"}),
            ("three", {"priority": "impossible"}),
        ])
    assert repository.snapshot() == before
"""
    return {"python": run_python_test(workspace, source)}


verify("dev-task-api-bulk-update", check)
