from typing import Any

from event_worker.store import EventStore


class EventConsumer:
    def __init__(self, store: EventStore, *, max_attempts: int = 3) -> None:
        self.store = store
        self.max_attempts = max_attempts

    def consume(self, event: dict[str, Any]) -> str:
        event_id = str(event.get("event_id", "")).strip()
        payload = event.get("payload")
        if (
            not event_id
            or event.get("type") != "task.updated"
            or not isinstance(payload, dict)
            or not payload.get("task_id")
            or not payload.get("actor_id")
        ):
            self.store.quarantine(event, "invalid v2 event")
            return "quarantined"
        if self.store.was_processed(event_id):
            return "duplicate"
        self.store.record(event)
        return "processed"
