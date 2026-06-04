import json
from datetime import UTC, datetime
from pathlib import Path

from app.metrics import calculate_metrics
from app.models import EvaluationRun, RunStatus
from app.storage import SQLiteStorage
from app.worker import EvalWorker


def write_tasks(path: Path) -> None:
    path.write_text(
        json.dumps(
            [
                {
                    "id": "task-pass",
                    "title": "Passing task",
                    "prompt": "Make a safe change.",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                    "expected_success": True,
                }
            ]
        ),
        encoding="utf-8",
    )


def make_pending_run(run_id: str) -> EvaluationRun:
    now = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    return EvaluationRun(
        run_id=run_id,
        status=RunStatus.PENDING,
        provider="mock",
        model="deterministic-agent-v1",
        case_source="tasks.json",
        requested_task_ids=["task-pass"],
        traces=[],
        metrics=calculate_metrics([]),
        started_at=now,
        ended_at=now,
        duration_ms=0,
    )


def test_worker_claims_pending_run_executes_tasks_and_persists_artifacts(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))

    worker = EvalWorker(storage=storage, forge_command=None)
    completed = worker.run_once()

    assert completed is not None
    assert completed.status == RunStatus.COMPLETED
    assert completed.traces[0].task_id == "task-pass"
    assert completed.metrics.success_rate == 1.0

    restarted = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    fetched = restarted.get_run("run-1")
    assert fetched is not None
    assert fetched.status == RunStatus.COMPLETED
    assert fetched.traces[0].provider == "mock"
    assert {artifact.kind for artifact in restarted.list_artifacts("run-1")} == {
        "report",
        "trace",
    }


def test_worker_returns_none_when_no_pending_runs(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )

    worker = EvalWorker(storage=storage, forge_command=None)

    assert worker.run_once() is None
