from pathlib import Path
from typing import Any

from _support import run_python_test, run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    python_source = r"""
import os
import sys
from pathlib import Path
import pytest

root = Path(os.environ["FORGEBENCH_WORKSPACE"])
sys.path.insert(0, str(root / "services" / "task-api"))
from task_api.app import TaskApi
from task_api.models import TaskRecord
from task_api.repository import TaskRepository

def test_endpoint_commits_all_or_restores_all():
    repository = TaskRepository([
        TaskRecord(id="one", title="One", revision=2),
        TaskRecord(id="two", title="Two", revision=4),
    ])
    api = TaskApi(repository)
    before = repository.snapshot()
    with pytest.raises(ValueError):
        api.patch_tasks([
            {"task_id": "one", "changes": {"status": "in_progress"}, "expected_revision": 2},
            {"task_id": "two", "changes": {"priority": "invalid"}, "expected_revision": 4},
        ])
    assert repository.snapshot() == before
    result = api.patch_tasks([
        {"task_id": "one", "changes": {"status": "in_progress"}, "expected_revision": 2},
        {"task_id": "two", "changes": {"priority": "urgent"}, "expected_revision": 4},
    ])
    assert [item["revision"] for item in result] == [3, 5]
"""
    typescript_source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { buildBulkUpdateRequest } = await import(
  `${root}/apps/admin-console/src/bulk-actions.ts`
);
const tasks = [
  { id: "one", title: "One", status: "todo", priority: "low", revision: 2 },
  { id: "two", title: "Two", status: "todo", priority: "high", revision: 4 },
];
test("admin request is authorized and carries conditional revisions", () => {
  expect(() => buildBulkUpdateRequest(tasks, "in_progress", "viewer")).toThrow();
  expect(buildBulkUpdateRequest(tasks, "in_progress", "operator")).toEqual({
    updates: [
      { task_id: "one", changes: { status: "in_progress" }, expected_revision: 2 },
      { task_id: "two", changes: { status: "in_progress" }, expected_revision: 4 },
    ],
  });
});
"""
    return {
        "python": run_python_test(workspace, python_source),
        "typescript": run_typescript_test(workspace, typescript_source),
    }


verify("h18", check)
