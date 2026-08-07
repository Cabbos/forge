from pathlib import Path
from typing import Any

from _support import run_python_test, run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    python_source = r"""
import os
import sys
from pathlib import Path

root = Path(os.environ["FORGEBENCH_WORKSPACE"])
sys.path.insert(0, str(root / "services" / "task-api"))
sys.path.insert(0, str(root / "services" / "event-worker"))
from event_worker.consumer import EventConsumer
from event_worker.store import EventStore
from task_api.app import TaskApi
from task_api.models import TaskRecord
from task_api.repository import TaskRepository

def test_audit_is_optional_and_survives_the_event_pipeline():
    api = TaskApi(TaskRepository([TaskRecord(id="one", title="One")]))
    audited = api.patch_task(
        "one", {"status": "in_progress"}, if_match=1, audit_id="audit-9", actor="alice"
    )
    assert audited["audit_id"] == "audit-9"
    assert audited["actor"] == "alice"

    legacy_api = TaskApi(TaskRepository([TaskRecord(id="legacy", title="Legacy")]))
    legacy = legacy_api.patch_task("legacy", {"priority": "high"}, if_match=1)
    assert "audit_id" not in legacy
    assert "actor" not in legacy

    store = EventStore()
    consumer = EventConsumer(store)
    modern_event = {
        "event_id": "modern",
        "type": "task.updated",
        "payload": {
            "task_id": audited["id"],
            "revision": audited["revision"],
            "audit_id": audited["audit_id"],
            "actor": audited["actor"],
        },
    }
    legacy_event = {
        "event_id": "legacy",
        "type": "task.updated",
        "payload": {"task_id": "legacy", "revision": 2},
    }
    assert consumer.consume(modern_event) == "processed"
    assert consumer.consume(legacy_event) == "processed"
    assert store.task_events[0]["payload"]["audit_id"] == "audit-9"
    assert store.task_events[0]["payload"]["actor"] == "alice"
    assert store.task_events[1]["payload"] == {"task_id": "legacy", "revision": 2}
"""
    typescript_source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { taskList } = await import(`${root}/apps/web-console/src/task-list.tsx`);
test("console rows expose optional audit facts", () => {
  const rows = taskList([
    {
      id: "one", title: "One", status: "todo", priority: "high", revision: 2,
      auditId: "audit-9", auditActor: "alice",
    },
    {
      id: "legacy", title: "Legacy", status: "todo", priority: "low", revision: 1,
    },
  ], "").rows;
  const modern = rows.find((row) => row.id === "one") as any;
  const legacy = rows.find((row) => row.id === "legacy") as any;
  expect(modern.auditId).toBe("audit-9");
  expect(modern.auditActor).toBe("alice");
  expect(legacy.auditId).toBeUndefined();
  expect(legacy.auditActor).toBeUndefined();
});
"""
    return {
        "python": run_python_test(workspace, python_source),
        "typescript": run_typescript_test(workspace, typescript_source),
    }


verify("h20", check)
