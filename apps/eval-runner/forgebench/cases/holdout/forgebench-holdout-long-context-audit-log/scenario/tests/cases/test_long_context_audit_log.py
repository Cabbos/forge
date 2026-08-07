from event_worker.consumer import EventConsumer
from event_worker.store import EventStore
from task_api.app import TaskApi
from task_api.models import TaskRecord
from task_api.repository import TaskRepository


def test_audit_facts_survive_api_and_worker_storage() -> None:
    api = TaskApi(TaskRepository([TaskRecord(id="task-1", title="Ship")]))
    result = api.patch_task(
        "task-1",
        {"status": "in_progress"},
        if_match=1,
        audit_id="audit-7",
        actor="alice",
    )

    assert result["audit_id"] == "audit-7"
    assert result["actor"] == "alice"

    store = EventStore()
    consumer = EventConsumer(store)
    event = {
        "event_id": "event-7",
        "type": "task.updated",
        "payload": {
            "task_id": result["id"],
            "revision": result["revision"],
            "audit_id": result["audit_id"],
            "actor": result["actor"],
        },
    }
    assert consumer.consume(event) == "processed"
    assert store.task_events[0]["payload"]["audit_id"] == "audit-7"
    assert store.task_events[0]["payload"]["actor"] == "alice"
