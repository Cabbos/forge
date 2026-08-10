from event_worker.consumer import EventConsumer
from event_worker.store import EventStore
from task_api.models import TaskRecord
from task_api.repository import TaskRepository
from task_api.service import TaskService


def test_task_update_flows_into_worker_once() -> None:
    repository = TaskRepository([TaskRecord(id="task-1", title="Ship release")])
    updated = TaskService(repository).update("task-1", {"status": "in_progress"})
    event = {
        "event_id": f"task-1-r{updated.revision}",
        "type": "task.updated",
        "payload": {"task_id": updated.id, "revision": updated.revision},
    }
    store = EventStore()
    consumer = EventConsumer(store)

    assert consumer.consume(event) == "processed"
    assert consumer.consume(event) == "duplicate"
    assert store.task_events == [event]
