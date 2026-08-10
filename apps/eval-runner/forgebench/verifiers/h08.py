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
from task_api.service import RevisionConflictError, TaskService

def test_conditional_update_is_atomic_and_increments_once():
    repository = TaskRepository([TaskRecord(id="one", title="One", revision=4)])
    service = TaskService(repository)
    before = repository.snapshot()
    with pytest.raises(RevisionConflictError):
        service.update("one", {"title": "Stale"}, expected_revision=3)
    assert repository.snapshot() == before
    updated = service.update("one", {"title": "Fresh"}, expected_revision=4)
    assert updated.title == "Fresh"
    assert updated.revision == 5
"""
    return {"python": run_python_test(workspace, source)}


verify("h08", check)
