from typing import Any

from event_worker.store import EventStore


class EventConsumer:
    def __init__(self, store: EventStore, *, max_attempts: int = 3) -> None:
        self.store = store
        self.max_attempts = max_attempts

    def consume(self, event: dict[str, Any]) -> str:
        event_id = str(event.get("event_id", "")).strip()
        if event_id and self.store.was_processed(event_id):
            return "duplicate"
        self.store.record({"event_id": event_id or "missing", **event})
        return "processed"
