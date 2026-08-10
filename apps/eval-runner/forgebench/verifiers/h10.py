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

def test_event_identity_not_object_identity_controls_deduplication():
    store = EventStore()
    consumer = EventConsumer(store)
    first = {
        "event_id": "same-id",
        "type": "task.updated",
        "payload": {"task_id": "one", "revision": 2},
    }
    second = {
        "event_id": "".join(["same", "-", "id"]),
        "type": "task.updated",
        "payload": {"task_id": "one", "revision": 2},
    }
    assert first is not second
    assert consumer.consume(first) == "processed"
    assert consumer.consume(second) == "duplicate"
    assert len(store.task_events) == 1
    assert store.processed_ids == {"same-id"}
"""
    return {"python": run_python_test(workspace, source)}


verify("h10", check)
