from pathlib import Path

from fastapi.testclient import TestClient

from app.main import create_app
from app.storage import InMemoryStorage


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
