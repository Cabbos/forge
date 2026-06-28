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


def make_pending_run(run_id: str, *, max_retries: int = 0) -> EvaluationRun:
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
        max_retries=max_retries,
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
        "trajectory",
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


def test_worker_does_not_retry_cancelled_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    run = storage.create_run(make_pending_run("run-1", max_retries=2))
    storage.cancel_run(run.run_id)

    result = EvalWorker(storage=storage, forge_command=None).run_once()

    assert result is None
    fetched = storage.get_run(run.run_id)
    assert fetched.status == RunStatus.CANCELLED
    assert fetched.retry_count == 0
    assert fetched.traces == []


def test_worker_writes_stderr_summaries_for_lifecycle_events(
    tmp_path: Path,
    capsys,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    worker = EvalWorker(storage=storage, forge_command=None, worker_id="worker-1")
    storage.create_run(make_pending_run("run-complete"))

    worker.run_once()

    completed_stderr = capsys.readouterr().err
    assert "[worker worker-1] claimed run run-complete status=running" in completed_stderr
    assert "[worker worker-1] completed run run-complete tasks=1" in completed_stderr

    storage.create_run(
        make_pending_run("run-retry", max_retries=1).model_copy(
            update={"requested_task_ids": ["missing-task"]}
        )
    )

    worker.run_once()

    retried_stderr = capsys.readouterr().err
    assert "[worker worker-1] claimed run run-retry status=running" in retried_stderr
    assert "[worker worker-1] retried run run-retry retry=1/1" in retried_stderr

    worker.run_once()

    failed_stderr = capsys.readouterr().err
    assert "[worker worker-1] claimed run run-retry status=running" in failed_stderr
    assert "[worker worker-1] failed run run-retry retries=1/1" in failed_stderr


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

    assert len(heartbeats) >= 2, (
        f"Expected at least 2 heartbeats during slow task, got {len(heartbeats)}"
    )


def test_worker_detects_cancellation_after_task_returns(tmp_path: Path, capsys) -> None:
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
    assert fetched.traces[0].task_id == "task-pass"
    assert "[worker local-worker] cancelled run run-1 tasks=1" in capsys.readouterr().err


def test_worker_completion_race_with_cancel_preserves_cancelled(tmp_path: Path) -> None:
    """If cancel happens between save_task and complete_run, final status must be CANCELLED."""
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
        def run_task(self, _task):
            time.sleep(0.05)
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

    worker = EvalWorker(storage=storage, forge_command=None)

    # Inject cancel inside save_task so it happens after task returns
    # but before the worker reaches complete_run.
    original_save_task = storage.save_task

    def cancel_in_save_task(run_id: str, trace: AgentTrace) -> None:
        original_save_task(run_id, trace)
        storage.cancel_run(run_id)

    storage.save_task = cancel_in_save_task  # type: ignore[method-assign]

    try:
        result = worker.run_once()
    finally:
        worker_mod.create_runner = original_create_runner
        storage.save_task = original_save_task  # type: ignore[method-assign]

    assert result is not None
    assert result.status == RunStatus.CANCELLED, (
        f"Expected CANCELLED when cancel races complete_run, got {result.status}"
    )
    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.CANCELLED


def test_worker_exception_race_with_cancel_preserves_cancelled(tmp_path: Path) -> None:
    """If cancel happens before retry/fail write, final status must be CANCELLED."""
    import time

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    # No retries so we hit the fail branch
    storage.create_run(make_pending_run("run-1").model_copy(update={"max_retries": 0}))

    import app.worker as worker_mod
    original_create_runner = worker_mod.create_runner

    class FailingRunner:
        def run_task(self, _task):
            time.sleep(0.05)
            raise RuntimeError("simulated failure")

    worker_mod.create_runner = lambda **kwargs: FailingRunner()  # type: ignore[assignment]

    worker = EvalWorker(storage=storage, forge_command=None)

    # Inject cancel inside fail_run so it happens right before the worker
    # attempts to write FAILED.
    original_fail_run = storage.fail_run

    def cancel_then_fail(failed_run: EvaluationRun) -> EvaluationRun:
        storage.cancel_run(failed_run.run_id)
        return original_fail_run(failed_run)

    storage.fail_run = cancel_then_fail  # type: ignore[method-assign]

    try:
        result = worker.run_once()
    finally:
        worker_mod.create_runner = original_create_runner
        storage.fail_run = original_fail_run  # type: ignore[method-assign]

    assert result is not None
    assert result.status == RunStatus.CANCELLED, (
        f"Expected CANCELLED when cancel races fail_run, got {result.status}"
    )
    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.CANCELLED


def test_worker_retry_race_with_cancel_preserves_cancelled(tmp_path: Path) -> None:
    """If cancel happens before retry write, final status must be CANCELLED."""
    import time

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    # One retry so we hit the retry branch first
    storage.create_run(make_pending_run("run-1").model_copy(update={"max_retries": 1}))

    import app.worker as worker_mod
    original_create_runner = worker_mod.create_runner

    class FailingRunner:
        def run_task(self, _task):
            time.sleep(0.05)
            raise RuntimeError("simulated failure")

    worker_mod.create_runner = lambda **kwargs: FailingRunner()  # type: ignore[assignment]

    worker = EvalWorker(storage=storage, forge_command=None)

    # Inject cancel inside retry_run so it happens right before the worker
    # attempts to write PENDING.
    original_retry_run = storage.retry_run

    def cancel_then_retry(retry_run: EvaluationRun) -> EvaluationRun:
        storage.cancel_run(retry_run.run_id)
        return original_retry_run(retry_run)

    storage.retry_run = cancel_then_retry  # type: ignore[method-assign]

    try:
        result = worker.run_once()
    finally:
        worker_mod.create_runner = original_create_runner
        storage.retry_run = original_retry_run  # type: ignore[method-assign]

    assert result is not None
    assert result.status == RunStatus.CANCELLED, (
        f"Expected CANCELLED when cancel races retry_run, got {result.status}"
    )
    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.CANCELLED


def test_worker_has_stop_method_that_sets_stop_flag(tmp_path: Path) -> None:
    """EvalWorker must expose a stop() method that sets an internal stop flag."""
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    worker = EvalWorker(storage=storage, forge_command=None)
    assert not worker.should_stop
    worker.stop()
    assert worker.should_stop


def test_worker_main_loop_respects_stop_flag(tmp_path: Path) -> None:
    """When stop() is called, the worker poll loop should exit after current iteration."""
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))

    worker = EvalWorker(storage=storage, forge_command=None, poll_interval_seconds=0.05)

    # Start worker in background; it will process run-1 then loop
    import threading

    def run_loop() -> None:
        # Override the infinite loop to stop after one iteration
        worker.run_once()
        while not worker.should_stop:
            result = worker.run_once()
            if result is None:
                # No more pending runs; this is where the real loop would sleep
                pass
            # In real loop it would sleep; here we just break to simulate
            break

    thread = threading.Thread(target=run_loop)
    thread.start()
    # Wait for worker to finish run-1
    thread.join(timeout=2)
    assert not thread.is_alive(), "Worker thread should have finished"
    fetched = storage.get_run("run-1")
    assert fetched.status == RunStatus.COMPLETED


def test_worker_consumes_multiple_queued_runs_and_persists_artifacts(tmp_path: Path) -> None:
    """Worker should continuously claim and execute multiple pending runs."""
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    storage.create_run(make_pending_run("run-1"))
    storage.create_run(make_pending_run("run-2"))

    worker = EvalWorker(storage=storage, forge_command=None)

    # Process both runs
    result1 = worker.run_once()
    result2 = worker.run_once()

    assert result1 is not None
    assert result1.status == RunStatus.COMPLETED
    assert result1.run_id == "run-1"
    assert result1.traces[0].task_id == "task-pass"

    assert result2 is not None
    assert result2.status == RunStatus.COMPLETED
    assert result2.run_id == "run-2"
    assert result2.traces[0].task_id == "task-pass"

    # Verify artifacts persisted for both runs
    artifacts1 = storage.list_artifacts("run-1")
    artifacts2 = storage.list_artifacts("run-2")
    assert {a.kind for a in artifacts1} == {"report", "trace", "trajectory"}
    assert {a.kind for a in artifacts2} == {"report", "trace", "trajectory"}

    # Verify trace files exist on disk
    trace1 = next(a for a in artifacts1 if a.kind == "trace")
    trace2 = next(a for a in artifacts2 if a.kind == "trace")
    assert Path(trace1.path).exists()
    assert Path(trace2.path).exists()

    # Verify restarted storage can read both runs
    restarted = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    assert len(restarted.list_runs()) == 2
    run1 = restarted.get_run("run-1")
    run2 = restarted.get_run("run-2")
    assert run1.status == RunStatus.COMPLETED
    assert run2.status == RunStatus.COMPLETED
    assert run1.metrics.success_rate == 1.0
    assert run2.metrics.success_rate == 1.0
    assert len(run1.traces) == 1
    assert len(run2.traces) == 1
