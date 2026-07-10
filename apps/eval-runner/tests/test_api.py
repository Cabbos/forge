import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.config import Settings
from app.main import build_storage, create_app
from app.models import AgentTrace, FailureCategory, RunStatus, VerificationResult
from app.storage import InMemoryStorage, SQLiteStorage
from app.worker import EvalWorker


def write_tasks(path: Path) -> None:
    path.write_text(
        """
[
  {
    "id": "task-pass",
    "title": "Passing task",
    "prompt": "Make a safe change.",
    "context_files": ["src/app.py"],
    "verification_command": "pytest",
    "expected_success": true
  },
  {
    "id": "task-fail",
    "title": "Failing task",
    "prompt": "Simulate a verification failure.",
    "context_files": ["src/worker.py"],
    "verification_command": "pytest tests/test_worker.py",
    "expected_success": false
  }
]
""".strip(),
        encoding="utf-8",
    )


def test_api_creates_run_and_exposes_trace_and_metrics(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    health = client.get("/health")
    assert health.status_code == 200
    assert health.json()["status"] == "ok"

    tasks = client.get("/tasks")
    assert tasks.status_code == 200
    assert [task["id"] for task in tasks.json()] == ["task-pass", "task-fail"]

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass", "task-fail"], "provider": "mock", "model": "portfolio-v1"},
    )
    assert created.status_code == 201
    run_payload = created.json()
    run_id = run_payload["run_id"]
    assert run_payload["status"] == "completed"
    assert run_payload["metrics"]["success_rate"] == 0.5

    fetched_run = client.get(f"/runs/{run_id}")
    assert fetched_run.status_code == 200
    assert fetched_run.json()["run_id"] == run_id

    trace = client.get(f"/runs/{run_id}/trace")
    assert trace.status_code == 200
    assert [item["task_id"] for item in trace.json()] == ["task-pass", "task-fail"]
    assert trace.json()[0]["tool_calls"][0]["command"] == "read_context"

    metrics = client.get(f"/runs/{run_id}/metrics")
    assert metrics.status_code == 200
    assert metrics.json()["failure_categories"] == {"verification_failed": 1}


def test_api_returns_404_for_unknown_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tasks_path)))

    response = client.get("/runs/missing")

    assert response.status_code == 404
    assert response.json()["detail"] == "Run not found"


def test_api_can_create_forge_run_without_configured_command_as_traceable_failure(
    tmp_path: Path,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tasks_path)))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "forge", "model": "local-forge"},
    )

    assert created.status_code == 201
    payload = created.json()
    assert payload["status"] == "completed"
    assert payload["metrics"]["success_rate"] == 0.0
    assert payload["metrics"]["failure_categories"] == {"runner_error": 1}
    assert payload["traces"][0]["provider"] == "forge"
    assert payload["traces"][0]["error"] == "forge_command_not_configured"
    assert payload["traces"][0]["failure_category"] == "runner_error"


def test_api_rejects_unknown_provider(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tasks_path)))

    response = client.post(
        "/runs",
        json={
            "task_ids": ["task-pass"],
            "provider": "unknown-provider",
            "model": "model-a",
        },
    )

    assert response.status_code == 422


def test_api_can_load_eval_case_directory_and_expose_report(tmp_path: Path) -> None:
    case_dir = tmp_path / "eval_cases" / "small-edit-success"
    (case_dir / "fixture" / "src").mkdir(parents=True)
    (case_dir / "fixture" / "src" / "app.py").write_text("VALUE = 1\n", encoding="utf-8")
    (case_dir / "case.json").write_text(
        json.dumps(
            {
                "task": {
                    "id": "small-edit-success",
                    "title": "Small edit succeeds",
                    "prompt": "Make a focused edit.",
                    "fixture_path": "fixture",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                    "expected_files_changed": ["src/app.py"],
                }
            }
        ),
        encoding="utf-8",
    )
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tmp_path / "eval_cases")))

    tasks = client.get("/tasks")
    assert tasks.status_code == 200
    assert tasks.json()[0]["id"] == "small-edit-success"
    assert Path(tasks.json()[0]["fixture_path"]).is_absolute()

    created = client.post("/runs", json={"provider": "mock"})
    assert created.status_code == 201
    run_id = created.json()["run_id"]

    report = client.get(f"/runs/{run_id}/report")
    assert report.status_code == 200
    assert report.json()["success_rate"] == 1.0
    assert report.json()["tasks"][0]["task_id"] == "small-edit-success"


def test_api_with_sqlite_storage_persists_run_after_restart(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    db_path = tmp_path / "forge_eval.db"
    artifacts_path = tmp_path / "artifacts"
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=db_path,
        artifacts_path=artifacts_path,
    )
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    assert created.status_code == 201
    run_id = created.json()["run_id"]

    restarted_storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=db_path,
        artifacts_path=artifacts_path,
    )
    restarted_client = TestClient(create_app(storage=restarted_storage))

    fetched_run = restarted_client.get(f"/runs/{run_id}")
    assert fetched_run.status_code == 200
    assert fetched_run.json()["run_id"] == run_id
    assert fetched_run.json()["traces"][0]["task_id"] == "task-pass"

    trace = restarted_client.get(f"/runs/{run_id}/trace")
    assert trace.status_code == 200
    assert trace.json()[0]["provider"] == "mock"

    metrics = restarted_client.get(f"/runs/{run_id}/metrics")
    assert metrics.status_code == 200
    assert metrics.json()["success_rate"] == 1.0

    report = restarted_client.get(f"/runs/{run_id}/report")
    assert report.status_code == 200
    assert report.json()["tasks"][0]["task_id"] == "task-pass"


def test_api_with_sqlite_storage_lists_runs_and_artifacts_after_restart(
    tmp_path: Path,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    db_path = tmp_path / "forge_eval.db"
    artifacts_path = tmp_path / "artifacts"
    storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=db_path,
        artifacts_path=artifacts_path,
    )
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    assert created.status_code == 201
    run_id = created.json()["run_id"]

    restarted_storage = SQLiteStorage(
        tasks_path=tasks_path,
        db_path=db_path,
        artifacts_path=artifacts_path,
    )
    restarted_client = TestClient(create_app(storage=restarted_storage))

    runs = restarted_client.get("/runs")
    assert runs.status_code == 200
    assert [run["run_id"] for run in runs.json()] == [run_id]
    assert runs.json()[0]["metrics"]["success_rate"] == 1.0

    trace = restarted_client.get(f"/runs/{run_id}/trace")
    assert trace.status_code == 200
    assert trace.json()[0]["task_id"] == "task-pass"

    metrics = restarted_client.get(f"/runs/{run_id}/metrics")
    assert metrics.status_code == 200
    assert metrics.json()["success_rate"] == 1.0

    report = restarted_client.get(f"/runs/{run_id}/report")
    assert report.status_code == 200
    assert report.json()["tasks"][0]["task_id"] == "task-pass"

    artifacts = restarted_client.get(f"/runs/{run_id}/artifacts")
    assert artifacts.status_code == 200
    assert {artifact["kind"] for artifact in artifacts.json()} == {
        "report",
        "trace",
        "trajectory",
    }
    assert all(Path(artifact["path"]).exists() for artifact in artifacts.json())


def test_build_storage_uses_sqlite_when_configured(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    settings = Settings(
        storage_backend="sqlite",
        tasks_path=tasks_path,
        db_path=tmp_path / "forge_eval.db",
        artifacts_path=tmp_path / "artifacts",
    )

    storage = build_storage(settings)

    assert isinstance(storage, SQLiteStorage)


def test_api_can_create_queued_run_for_worker_execution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    monkeypatch.setenv("FORGE_EVAL_RUN_EXECUTION_MODE", "queued")
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )

    assert created.status_code == 201
    payload = created.json()
    run_id = payload["run_id"]
    assert payload["status"] == "pending"
    assert payload["traces"] == []
    assert payload["provider"] == "mock"
    assert payload["model"] == "portfolio-v1"

    completed = EvalWorker(storage=storage, forge_command=None).run_once()
    assert completed is not None
    assert completed.run_id == run_id

    fetched = client.get(f"/runs/{run_id}")
    assert fetched.status_code == 200
    assert fetched.json()["status"] == "completed"
    assert fetched.json()["traces"][0]["task_id"] == "task-pass"


def test_queue_status_counts_runs_by_status(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tasks_path)))
    client.post("/runs", json={"task_ids": ["task-pass"], "provider": "mock"})

    response = client.get("/queue/status")

    assert response.status_code == 200
    payload = response.json()
    assert payload["counts"]["completed"] >= 1
    assert "oldest_pending_run_id" in payload


def test_api_can_cancel_pending_run(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    monkeypatch.setenv("FORGE_EVAL_RUN_EXECUTION_MODE", "queued")
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    run_id = created.json()["run_id"]

    cancel = client.post(f"/runs/{run_id}/cancel")

    assert cancel.status_code == 200
    assert cancel.json()["status"] == "cancelled"
    fetched = client.get(f"/runs/{run_id}")
    assert fetched.json()["status"] == "cancelled"


def test_api_can_cancel_running_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    run_id = created.json()["run_id"]
    storage.update_run_status(run_id, RunStatus.RUNNING)

    cancel = client.post(f"/runs/{run_id}/cancel")

    assert cancel.status_code == 200
    assert cancel.json()["status"] == "cancelled"


def test_api_includes_failure_visibility_for_failed_run(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    run_id = created.json()["run_id"]
    failed_run = storage.get_run(run_id).model_copy(
        update={
            "status": RunStatus.FAILED,
            "failure_reason": "Worker crashed",
            "failure_category": FailureCategory.RUNNER_ERROR,
            "retry_count": 2,
            "max_retries": 2,
        }
    )
    storage.save_run(failed_run)

    fetched = client.get(f"/runs/{run_id}")
    payload = fetched.json()
    assert payload["status"] == "failed"
    assert payload["failure_reason"] == "Worker crashed"
    assert payload["failure_category"] == "runner_error"
    assert payload["retry_count"] == 2
    assert payload["max_retries"] == 2


def test_api_can_filter_runs_by_status(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    # Create three runs with different statuses
    completed = client.post("/runs", json={"task_ids": ["task-pass"], "provider": "mock"})
    run_completed = completed.json()["run_id"]

    pending = client.post("/runs", json={"task_ids": ["task-pass"], "provider": "mock"})
    run_pending = pending.json()["run_id"]
    storage.update_run_status(run_pending, RunStatus.PENDING)

    failed = client.post("/runs", json={"task_ids": ["task-pass"], "provider": "mock"})
    run_failed = failed.json()["run_id"]
    storage.update_run_status(run_failed, RunStatus.FAILED)

    # Filter by completed
    filtered = client.get("/runs?status=completed")
    assert filtered.status_code == 200
    ids = {r["run_id"] for r in filtered.json()}
    assert ids == {run_completed}

    # Filter by pending
    filtered = client.get("/runs?status=pending")
    assert filtered.status_code == 200
    ids = {r["run_id"] for r in filtered.json()}
    assert ids == {run_pending}

    # Filter by failed
    filtered = client.get("/runs?status=failed")
    assert filtered.status_code == 200
    ids = {r["run_id"] for r in filtered.json()}
    assert ids == {run_failed}

    # No filter returns all
    all_runs = client.get("/runs")
    assert all_runs.status_code == 200
    assert len(all_runs.json()) == 3


def test_api_queued_run_includes_max_retries(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    monkeypatch.setenv("FORGE_EVAL_RUN_EXECUTION_MODE", "queued")
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={
            "task_ids": ["task-pass"],
            "provider": "mock",
            "model": "portfolio-v1",
            "max_retries": 3,
        },
    )

    assert created.status_code == 201
    payload = created.json()
    assert payload["max_retries"] == 3
    fetched = storage.get_run(payload["run_id"])
    assert fetched.max_retries == 3


def test_api_cancel_completed_run_returns_409(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    run_id = created.json()["run_id"]
    storage.update_run_status(run_id, RunStatus.COMPLETED)

    cancel = client.post(f"/runs/{run_id}/cancel")

    assert cancel.status_code == 409
    fetched = client.get(f"/runs/{run_id}")
    assert fetched.json()["status"] == "completed"


def test_api_cancellation_during_task_preserves_cancelled(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """If a run is cancelled while a slow task is executing, the final status must be CANCELLED."""
    import threading
    import time
    from datetime import UTC, datetime

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    monkeypatch.setenv("FORGE_EVAL_RUN_EXECUTION_MODE", "queued")
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage))

    created = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock", "model": "portfolio-v1"},
    )
    run_id = created.json()["run_id"]

    import app.worker as worker_mod

    original_create_runner = worker_mod.create_runner

    class SlowRunner:
        def run_task(self, _task):
            time.sleep(0.3)
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
    worker_thread = threading.Thread(target=worker.run_once)
    worker_thread.start()
    time.sleep(0.1)  # Let worker start the slow task
    client.post(f"/runs/{run_id}/cancel")
    worker_thread.join(timeout=2)

    worker_mod.create_runner = original_create_runner

    fetched = client.get(f"/runs/{run_id}")
    assert fetched.json()["status"] == "cancelled", (
        f"Expected cancelled, got {fetched.json()['status']}. "
        "Run was cancelled during task execution but worker overwrote it with completed."
    )
