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

def test_optional_v2_fields_do_not_break_v1_producers():
    store = EventStore()
    consumer = EventConsumer(store)
    v1 = {
        "event_id": "v1-event",
        "type": "task.updated",
        "payload": {"task_id": "task-1", "revision": 2},
    }
    assert consumer.consume(v1) == "processed"
    assert consumer.consume(v1) == "duplicate"
    assert store.task_events == [v1]
"""
    return {"python": run_python_test(workspace, source)}


verify("dev-contract-event-schema", check)
