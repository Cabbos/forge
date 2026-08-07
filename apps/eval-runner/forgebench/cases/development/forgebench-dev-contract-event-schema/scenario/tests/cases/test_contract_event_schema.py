from event_worker.consumer import EventConsumer
from event_worker.store import EventStore


def test_v1_task_event_remains_consumable() -> None:
    event = {
        "event_id": "event-v1",
        "type": "task.updated",
        "payload": {"task_id": "task-1", "revision": 2},
    }
    store = EventStore()
    assert EventConsumer(store).consume(event) == "processed"
    assert store.task_events == [event]
