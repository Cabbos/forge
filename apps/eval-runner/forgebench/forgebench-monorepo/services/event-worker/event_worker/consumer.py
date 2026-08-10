from typing import Any

from event_worker.store import EventStore


class EventConsumer:
    def __init__(self, store: EventStore, *, max_attempts: int = 3) -> None:
        self.store = store
        self.max_attempts = max_attempts

    def consume(self, event: dict[str, Any]) -> str:
        event_id = str(event.get("event_id", "")).strip()
        if not event_id:
            self.store.quarantine(event, "missing event_id")
            return "quarantined"
        if self.store.was_processed(event_id):
            return "duplicate"
        if event.get("type") != "task.updated":
            self.store.quarantine(event, "unsupported event type")
            return "quarantined"
        payload = event.get("payload")
        if not isinstance(payload, dict) or not payload.get("task_id"):
            self.store.quarantine(event, "invalid payload")
            return "quarantined"
        attempt = int(event.get("attempt", 1))
        if attempt > self.max_attempts:
            self.store.quarantine(event, "retry limit exceeded")
            return "quarantined"
        self.store.record(event)
        return "processed"
