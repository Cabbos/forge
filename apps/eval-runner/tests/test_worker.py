import json
from datetime import UTC, datetime
from pathlib import Path

from app.metrics import calculate_metrics
from app.models import AgentTrace, EvaluationRun, RunStatus, VerificationResult
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


def test_worker_skips_cancelled_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))
    storage.cancel_run("run-1")

    worker = EvalWorker(storage=storage, forge_command=None)
    result = worker.run_once()

    # Cancelled runs should not be claimed or processed
    assert result is None
    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.CANCELLED


def test_worker_retries_failed_run_then_marks_terminal(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    # Create a run that will fail (no matching task will cause runner error)
    bad_run = make_pending_run("run-1").model_copy(
        update={"requested_task_ids": ["nonexistent-task"], "max_retries": 1}
    )
    storage.create_run(bad_run)

    worker = EvalWorker(storage=storage, forge_command=None)

    # First attempt fails and retries back to pending
    result1 = worker.run_once()
    assert result1 is not None
    assert result1.status == RunStatus.PENDING
    assert result1.retry_count == 1

    # Second attempt exhausts retry and marks FAILED
    result2 = worker.run_once()
    assert result2 is not None
    assert result2.status == RunStatus.FAILED
    assert result2.retry_count == 1
    assert result2.failure_reason is not None


def test_worker_heartbeats_during_long_task(tmp_path: Path) -> None:
    """Background heartbeat should refresh lease while a long task is running."""
    import threading
    import time
    from datetime import UTC, datetime

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))

    heartbeats: list[datetime] = []
    original_heartbeat = storage.heartbeat_run

    def spy_heartbeat(run_id: str, worker_id: str, lease_expires_at: datetime) -> None:
        heartbeats.append(datetime.now(UTC))
        original_heartbeat(run_id, worker_id, lease_expires_at)

    storage.heartbeat_run = spy_heartbeat  # type: ignore[method-assign]

    # Monkey-patch runner to make task slow
    import app.worker as worker_mod
    original_create_runner = worker_mod.create_runner

    class SlowRunner:
        def run_task(self, _task):
            time.sleep(0.5)
            from app.models import AgentTrace, VerificationResult
            return AgentTrace(
                task_id="task-pass",
                user_prompt="pass",
                model="mock",
                provider="mock",
                final_answer="done",
                verification_result=VerificationResult(
                    command="pytest", passed=True, exit_code=0, duration_ms=10
                ),
                started_at=datetime.now(UTC),
                ended_at=datetime.now(UTC),
                duration_ms=10,
            )

    worker_mod.create_runner = lambda **kwargs: SlowRunner()  # type: ignore[assignment]

    try:
        worker = EvalWorker(
            storage=storage,
            forge_command=None,
            heartbeat_interval_seconds=0.1,
        )
        worker.run_once()
    finally:
        worker_mod.create_runner = original_create_runner

    assert len(heartbeats) >= 2, f"Expected at least 2 heartbeats during slow task, got {len(heartbeats)}"


def test_worker_detects_cancellation_after_task_returns(tmp_path: Path) -> None:
    """If a run is cancelled DURING task execution, worker must not overwrite with COMPLETED."""
    import threading
    import time
    from datetime import UTC, datetime

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))

    import app.worker as worker_mod
    original_create_runner = worker_mod.create_runner

    class SlowRunner:
        def __init__(self, storage_ref) -> None:
            self.storage_ref = storage_ref

        def run_task(self, _task):
            time.sleep(0.2)
            return AgentTrace(
                task_id="task-pass",
                user_prompt="pass",
                model="mock",
                provider="mock",
                final_answer="done",
                verification_result=VerificationResult(
                    command="pytest", passed=True, exit_code=0, duration_ms=10
                ),
                started_at=datetime.now(UTC),
                ended_at=datetime.now(UTC),
                duration_ms=10,
            )

    worker_mod.create_runner = lambda **kwargs: SlowRunner(storage)  # type: ignore[assignment]

    worker = EvalWorker(
        storage=storage,
        forge_command=None,
        heartbeat_interval_seconds=0.05,
    )

    # Start worker in background thread
    worker_thread = threading.Thread(target=worker.run_once)
    worker_thread.start()
    time.sleep(0.1)  # Let worker start the slow task
    storage.cancel_run("run-1")
    worker_thread.join(timeout=3)

    worker_mod.create_runner = original_create_runner

    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.CANCELLED, (
        f"Expected cancelled after task returned, got {fetched.status}"
    )
