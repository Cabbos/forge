import json
from pathlib import Path
from typing import Any

from app.models import BacktestReport, ReportArtifact


def _coerce_report_payload(payload: dict[str, Any]) -> tuple[dict[str, Any], int]:
    if "report" in payload and isinstance(payload["report"], dict):
        traces = payload.get("traces", [])
        trace_count = len(traces) if isinstance(traces, list) else 0
        return payload["report"], trace_count
    return payload, 0


def load_report_artifact(path: Path) -> ReportArtifact:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Report artifact must be a JSON object: {path}")
    report_payload, trace_count = _coerce_report_payload(payload)
    experiment = payload.get("experiment", {})
    return ReportArtifact(
        path=path,
        report=BacktestReport.model_validate(report_payload),
        trace_count=trace_count,
        experiment=experiment if isinstance(experiment, dict) else {},
    )
