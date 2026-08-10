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
from task_api.models import TaskRecord, validate_task

def test_every_domain_constraint_is_validated():
    invalid = [
        TaskRecord(id="1", title="   "),
        TaskRecord(id="1", title="One", status="paused"),
        TaskRecord(id="1", title="One", priority="critical"),
        TaskRecord(id="1", title="One", revision=0),
    ]
    for task in invalid:
        with pytest.raises(ValueError):
            validate_task(task)
    validate_task(TaskRecord(id="1", title="One", status="done", priority="urgent", revision=2))
"""
    return {"python": run_python_test(workspace, source)}


verify("h07", check)
