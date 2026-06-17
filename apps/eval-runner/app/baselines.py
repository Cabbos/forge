import json
from datetime import UTC, datetime
from pathlib import Path

from app.artifacts import load_report_artifact
from app.models import BaselineRecord, BaselineRegistryPayload


class BaselineRegistry:
    def __init__(self, path: Path) -> None:
        self.path = path

    def promote(
        self,
        *,
        artifact_path: Path,
        name: str,
        trusted: bool = True,
        note: str | None = None,
    ) -> BaselineRecord:
        artifact = load_report_artifact(artifact_path)
        experiment = artifact.experiment
        record = BaselineRecord(
            name=name,
            artifact_path=str(artifact_path),
            promoted_at=datetime.now(UTC),
            trusted=trusted,
            dataset_fingerprint=experiment.get("dataset_fingerprint"),
            provider=experiment.get("provider"),
            model=experiment.get("model"),
            success_rate=artifact.report.success_rate,
            scope_violation_rate=artifact.report.scope_violation_rate,
            note=note,
        )
        payload = self.load()
        payload.records.append(record)
        self.save(payload)
        return record

    def latest(self, *, name: str) -> BaselineRecord | None:
        records = [
            record for record in self.load().records if record.name == name and record.trusted
        ]
        return records[-1] if records else None

    def load(self) -> BaselineRegistryPayload:
        if not self.path.exists():
            return BaselineRegistryPayload()
        return BaselineRegistryPayload.model_validate_json(
            self.path.read_text(encoding="utf-8")
        )

    def save(self, payload: BaselineRegistryPayload) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(
            json.dumps(payload.model_dump(mode="json"), indent=2),
            encoding="utf-8",
        )
