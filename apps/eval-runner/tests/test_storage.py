import json
import sqlite3
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path

import pytest

from app.metrics import calculate_metrics
from app.models import (
    AgentTrace,
    EvalArtifact,
    EvaluationRun,
    FailureCategory,
    RunStatus,
    ShellOutput,
    VerificationResult,
)
from app.storage import InMemoryStorage, RunAlreadyTerminalError, SQLiteStorage

StorageFactory = Callable[[Path, Path, Path], object]


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


def make_trace(task_id: str, *, raw_marker: str = "trace-marker") -> AgentTrace:
    started_at = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    ended_at = datetime(2026, 6, 4, 10, 0, 1, tzinfo=UTC)
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Run {task_id}",
        model="deterministic-agent-v1",
        provider="mock",
        context_files=["src/app.py"],
        raw_events=[{"event_type": "text_chunk", "content": raw_marker}],
        tool_calls=[ShellOutput(command="read_context")],
        shell_outputs=[],
        file_diffs=[],
        changed_files=["src/app.py"],
        scope_violations=[],
        final_answer="done",
        verification_result=VerificationResult(
            command="pytest",
            passed=True,
            stdout="1 passed",
            stderr="",
            exit_code=0,
            duration_ms=120,
        ),
        error=None,
        failure_reason=None,
        failure_category=FailureCategory.NONE,
        model_rounds=2,
        confirm_requests=1,
        started_at=started_at,
        ended_at=ended_at,
        duration_ms=1000,
    )


def make_run(run_id: str, traces: list[AgentTrace] | None = None) -> EvaluationRun:
    started_at = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    ended_at = datetime(2026, 6, 4, 10, 0, 1, tzinfo=UTC)
    run_traces = traces or []
    return EvaluationRun(
        run_id=run_id,
        status=RunStatus.RUNNING,
        provider="mock",
        model="deterministic-agent-v1",
        case_source="tasks.json",
        requested_task_ids=["task-pass"],
        traces=run_traces,
        metrics=calculate_metrics(run_traces),
        started_at=started_at,
        ended_at=ended_at,
        duration_ms=1000,
    )


def make_artifact(artifacts_path: Path, run_id: str, *, kind: str = "stdout") -> EvalArtifact:
    artifact_path = artifacts_path / run_id / f"{kind}.txt"
    artifact_path.parent.mkdir(parents=True, exist_ok=True)
    artifact_path.write_text(f"{kind} artifact", encoding="utf-8")
    return EvalArtifact(
        id=f"{run_id}-{kind}",
        run_id=run_id,
        kind=kind,
        path=str(artifact_path),
        size_bytes=artifact_path.stat().st_size,
        created_at=datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC),
    )


def storage_factories() -> list[tuple[str, StorageFactory]]:
    return [
        (
            "memory",
            lambda tasks_path, _db_path, _artifacts_path: InMemoryStorage(tasks_path=tasks_path),
        ),
        (
            "sqlite",
            lambda tasks_path, db_path, artifacts_path: SQLiteStorage(
                tasks_path=tasks_path,
                db_path=db_path,
                artifacts_path=artifacts_path,
            ),
        ),
    ]


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_saves_runs_tasks_and_artifact_metadata(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    artifacts_path = tmp_path / f"{storage_name}-artifacts"
    storage = storage_factory(tasks_path, tmp_path / f"{storage_name}.db", artifacts_path)

    storage.create_run(make_run("run-1"))
    storage.save_task("run-1", make_trace("task-pass"))
    storage.save_artifact(make_artifact(artifacts_path, "run-1"))
    storage.update_run_status("run-1", RunStatus.COMPLETED)

    fetched = storage.get_run("run-1")
    assert fetched is not None
    assert fetched.status == RunStatus.COMPLETED
    assert fetched.traces[0].task_id == "task-pass"
    assert fetched.traces[0].raw_events[0]["content"] == "trace-marker"
    assert fetched.metrics.total_tasks == 1
    assert fetched.metrics.success_rate == 1.0
    assert [run.run_id for run in storage.list_runs()] == ["run-1"]
    assert "stdout" in {artifact.kind for artifact in storage.list_artifacts("run-1")}


def test_sqlite_storage_survives_restart_and_keeps_large_trace_json_out_of_db(
    tmp_path: Path,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    db_path = tmp_path / "forge_eval.db"
    artifacts_path = tmp_path / "artifacts"
    raw_marker = "large-raw-event-sentinel"

    storage = SQLiteStorage(tasks_path=tasks_path, db_path=db_path, artifacts_path=artifacts_path)
    storage.save_run(make_run("run-1", [make_trace("task-pass", raw_marker=raw_marker)]))
    storage.save_artifact(make_artifact(artifacts_path, "run-1", kind="stdout"))

    restarted = SQLiteStorage(tasks_path=tasks_path, db_path=db_path, artifacts_path=artifacts_path)
    fetched = restarted.get_run("run-1")

    assert fetched is not None
    assert fetched.traces[0].raw_events[0]["content"] == raw_marker
    assert fetched.metrics.success_rate == 1.0
    assert {artifact.kind for artifact in restarted.list_artifacts("run-1")} == {
        "report",
        "stdout",
        "trace",
    }

    with sqlite3.connect(db_path) as connection:
        stored_text = "\n".join(
            str(row)
            for table in ("eval_runs", "eval_run_tasks", "eval_artifacts")
            for row in connection.execute(f"SELECT * FROM {table}").fetchall()
        )
    assert raw_marker not in stored_text
    assert raw_marker in (artifacts_path / "run-1" / "trace.json").read_text(encoding="utf-8")


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_claims_only_pending_runs(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    storage.create_run(make_run("completed-run").model_copy(update={"status": RunStatus.COMPLETED}))
    storage.create_run(make_run("pending-run").model_copy(update={"status": RunStatus.PENDING}))

    claimed = storage.claim_pending_run()

    assert claimed is not None
    assert claimed.run_id == "pending-run"
    assert claimed.status == RunStatus.RUNNING
    assert storage.get_run("pending-run").status == RunStatus.RUNNING
    assert storage.claim_pending_run() is None


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_cancels_pending_run(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    storage.create_run(make_run("pending-run").model_copy(update={"status": RunStatus.PENDING}))

    cancelled = storage.cancel_run("pending-run")

    assert cancelled is not None
    assert cancelled.status == RunStatus.CANCELLED
    assert storage.get_run("pending-run").status == RunStatus.CANCELLED


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_cancel_running_run_marks_cancellation_requested(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    storage.create_run(make_run("running-run").model_copy(update={"status": RunStatus.RUNNING}))

    cancelled = storage.cancel_run("running-run")

    assert cancelled is not None
    assert cancelled.status == RunStatus.CANCELLED
    assert storage.get_run("running-run").status == RunStatus.CANCELLED


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_claim_writes_lease(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    storage.create_run(make_run("pending-run").model_copy(update={"status": RunStatus.PENDING}))

    claimed = storage.claim_pending_run(worker_id="worker-1")

    assert claimed is not None
    assert claimed.worker_id == "worker-1"
    assert claimed.claimed_at is not None
    assert claimed.lease_expires_at is not None
    fetched = storage.get_run("pending-run")
    assert fetched.worker_id == "worker-1"
    assert fetched.claimed_at is not None


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_heartbeat_extends_lease(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    storage.create_run(make_run("running-run").model_copy(update={"status": RunStatus.RUNNING}))
    future = datetime(2099, 1, 1, 0, 0, 0, tzinfo=UTC)

    storage.heartbeat_run("running-run", worker_id="worker-1", lease_expires_at=future)

    fetched = storage.get_run("running-run")
    assert fetched.heartbeat_at is not None
    assert fetched.lease_expires_at is not None
    assert fetched.lease_expires_at == future


def test_sqlite_storage_can_reclaim_expired_lease_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )
    past = datetime(2000, 1, 1, 0, 0, 0, tzinfo=UTC)
    storage.create_run(
        make_run("stale-run").model_copy(
            update={
                "status": RunStatus.RUNNING,
                "worker_id": "old-worker",
                "lease_expires_at": past,
            }
        )
    )

    claimed = storage.claim_pending_run(worker_id="new-worker")

    assert claimed is not None
    assert claimed.run_id == "stale-run"
    assert claimed.worker_id == "new-worker"
    assert claimed.status == RunStatus.RUNNING


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_cancel_terminal_run_is_noop(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    for terminal_status in [RunStatus.COMPLETED, RunStatus.FAILED, RunStatus.CANCELLED]:
        run_id = f"run-{terminal_status.value}"
        storage.create_run(make_run(run_id).model_copy(update={"status": terminal_status}))

        with pytest.raises(RunAlreadyTerminalError):
            storage.cancel_run(run_id)

        fetched = storage.get_run(run_id)
        assert fetched.status == terminal_status
