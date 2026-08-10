from pathlib import Path
from typing import Any

from _support import run_python_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import os
import sys
from pathlib import Path

root = Path(os.environ["FORGEBENCH_WORKSPACE"])
sys.path.insert(0, str(root / "services" / "event-worker"))
from event_worker.consumer import EventConsumer
from event_worker.store import EventStore

def test_duplicate_event_has_no_second_side_effect():
    store = EventStore()
    consumer = EventConsumer(store)
    event = {
        "event_id": "same-event",
        "type": "task.updated",
        "payload": {"task_id": "task-1", "revision": 9},
    }
    assert consumer.consume(event) == "processed"
    first_snapshot = list(store.task_events)
    assert consumer.consume(dict(event)) == "duplicate"
    assert store.task_events == first_snapshot
    assert len(store.processed_ids) == 1
"""
    return {"python": run_python_test(workspace, source)}


verify("dev-idempotent-no-op", check)
