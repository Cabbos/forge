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

def test_poison_events_are_quarantined_with_reasons():
    store = EventStore()
    consumer = EventConsumer(store, max_attempts=2)
    events = [
        {"type": "task.updated", "payload": {"task_id": "one"}},
        {"event_id": "wrong", "type": "unknown", "payload": {"task_id": "one"}},
        {"event_id": "bad-payload", "type": "task.updated", "payload": {}},
        {
            "event_id": "exhausted",
            "type": "task.updated",
            "attempt": 3,
            "payload": {"task_id": "one"},
        },
    ]
    assert [consumer.consume(event) for event in events] == ["quarantined"] * 4
    assert store.task_events == []
    assert len(store.quarantined) == 4
    assert all(item["reason"].strip() for item in store.quarantined)
"""
    return {"python": run_python_test(workspace, source)}


verify("h09", check)
