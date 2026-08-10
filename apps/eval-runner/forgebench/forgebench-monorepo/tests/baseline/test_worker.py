from event_worker.consumer import EventConsumer
from event_worker.retry import retry_delay_seconds
from event_worker.store import EventStore


def test_worker_processes_once_and_quarantines_poison_messages() -> None:
    store = EventStore()
    consumer = EventConsumer(store)
    event = {
        "event_id": "event-1",
        "type": "task.updated",
        "payload": {"task_id": "task-1", "revision": 2},
    }

    assert consumer.consume(event) == "processed"
    assert consumer.consume(event) == "duplicate"
    assert consumer.consume({"event_id": "event-2", "type": "unknown"}) == "quarantined"
    assert len(store.task_events) == 1
    assert len(store.quarantined) == 1


def test_worker_retry_delay_is_capped_without_off_by_one() -> None:
    assert [retry_delay_seconds(attempt, base=1, cap=5) for attempt in range(1, 6)] == [
        1,
        2,
        4,
        5,
        5,
    ]
