from typing import Any


class EventStore:
    def __init__(self) -> None:
        self.processed_ids: set[str] = set()
        self.task_events: list[dict[str, Any]] = []
        self.quarantined: list[dict[str, Any]] = []

    def was_processed(self, event_id: str) -> bool:
        return False

    def record(self, event: dict[str, Any]) -> None:
        self.task_events.append(dict(event))
        self.processed_ids.add(str(event["event_id"]))

    def quarantine(self, event: dict[str, Any], reason: str) -> None:
        self.quarantined.append({"event": dict(event), "reason": reason})
