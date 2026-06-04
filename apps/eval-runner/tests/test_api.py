import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.config import Settings
from app.main import build_storage, create_app
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
