"""End-to-end smoke tests for the queued worker + SQLite service mode.

These tests verify the full lifecycle:
  API create run -> worker claim -> execute -> trace/report/artifact persist -> API query.
"""

import json
import subprocess
import sys
import time
from pathlib import Path

import httpx
import pytest


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
                },
                {
                    "id": "task-fail",
                    "title": "Failing task",
                    "prompt": "Simulate a verification failure.",
                    "context_files": ["src/worker.py"],
                    "verification_command": "pytest tests/test_worker.py",
                    "expected_success": False,
                },
            ]
        ),
        encoding="utf-8",
    )


def _wait_for_url(url: str, timeout: float = 10.0) -> None:
    """Poll until the service is reachable or timeout."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = httpx.get(url, timeout=1)
            if resp.status_code == 200:
                return
        except httpx.RequestError:
            pass
        time.sleep(0.1)
    raise TimeoutError(f"Service at {url} did not become ready within {timeout}s")


def test_golden_harness_check_passes_for_expected_success_cases(tmp_path: Path) -> None:
    from app.harness_checks import run_golden_harness_check

    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)

    assert run_golden_harness_check(tasks_path) is True


@pytest.mark.slow
class TestQueuedServiceSmoke:
    """Smoke tests that spin up the real FastAPI service and worker process."""

    def test_queued_run_flow_service_plus_worker(self, tmp_path: Path) -> None:
        """Full lifecycle: service creates queued run, worker executes, API queries results."""
        tasks_path = tmp_path / "tasks.json"
        write_tasks(tasks_path)
        db_path = tmp_path / "forge_eval.db"
        artifacts_path = tmp_path / "artifacts"
        port = 18765  # Unlikely to conflict
        base_url = f"http://127.0.0.1:{port}"

        env = {
            **dict(subprocess.os.environ),
            "FORGE_EVAL_STORAGE_BACKEND": "sqlite",
            "FORGE_EVAL_DB_PATH": str(db_path),
            "FORGE_EVAL_ARTIFACTS_PATH": str(artifacts_path),
            "FORGE_EVAL_TASKS_PATH": str(tasks_path),
            "FORGE_EVAL_RUN_EXECUTION_MODE": "queued",
        }

        # 1. Start the FastAPI service
        server = subprocess.Popen(
            [sys.executable, "-m", "uvicorn", "app.main:app", "--port", str(port)],
            cwd=str(Path(__file__).parent.parent),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            _wait_for_url(f"{base_url}/health")

            # 2. Create a queued run via API
            create_resp = httpx.post(
                f"{base_url}/runs",
                json={
                    "task_ids": ["task-pass", "task-fail"],
                    "provider": "mock",
                    "model": "smoke-v1",
                },
                timeout=5,
            )
            assert create_resp.status_code == 201
            run = create_resp.json()
            run_id = run["run_id"]
            assert run["status"] == "pending"
            assert run["traces"] == []

            # 3. Run the worker once
            worker = subprocess.run(
                [sys.executable, "-m", "app.worker", "--once"],
                cwd=str(Path(__file__).parent.parent),
                env=env,
                capture_output=True,
                text=True,
                timeout=30,
            )
            assert worker.returncode == 0, f"Worker failed: {worker.stderr}"

            # 4. Query the run — should be completed
            run_resp = httpx.get(f"{base_url}/runs/{run_id}", timeout=5)
            assert run_resp.status_code == 200
            run_data = run_resp.json()
            assert run_data["status"] == "completed"
            assert run_data["provider"] == "mock"
            assert len(run_data["traces"]) == 2
            assert run_data["metrics"]["success_rate"] == 0.5

            # 5. Query trace
            trace_resp = httpx.get(f"{base_url}/runs/{run_id}/trace", timeout=5)
            assert trace_resp.status_code == 200
            traces = trace_resp.json()
            assert len(traces) == 2
            assert traces[0]["task_id"] == "task-pass"
            assert traces[1]["task_id"] == "task-fail"

            # 6. Query metrics
            metrics_resp = httpx.get(f"{base_url}/runs/{run_id}/metrics", timeout=5)
            assert metrics_resp.status_code == 200
            metrics = metrics_resp.json()
            assert metrics["total_tasks"] == 2
            assert metrics["passed_tasks"] == 1
            assert metrics["failed_tasks"] == 1

            # 7. Query report
            report_resp = httpx.get(f"{base_url}/runs/{run_id}/report", timeout=5)
            assert report_resp.status_code == 200
            report = report_resp.json()
            assert report["success_rate"] == 0.5
            assert len(report["tasks"]) == 2

            # 8. Query artifacts
            artifacts_resp = httpx.get(f"{base_url}/runs/{run_id}/artifacts", timeout=5)
            assert artifacts_resp.status_code == 200
            artifacts = artifacts_resp.json()
            assert {a["kind"] for a in artifacts} == {"report", "trace"}
            for artifact in artifacts:
                assert Path(artifact["path"]).exists()

            # 9. Verify status filtering on /runs
            completed_resp = httpx.get(f"{base_url}/runs?status=completed", timeout=5)
            assert completed_resp.status_code == 200
            assert any(r["run_id"] == run_id for r in completed_resp.json())

            pending_resp = httpx.get(f"{base_url}/runs?status=pending", timeout=5)
            assert pending_resp.status_code == 200
            assert not any(r["run_id"] == run_id for r in pending_resp.json())
        finally:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()

    def test_worker_continuous_consumption(self, tmp_path: Path) -> None:
        """Worker should consume multiple queued runs in a single session."""
        tasks_path = tmp_path / "tasks.json"
        write_tasks(tasks_path)
        db_path = tmp_path / "forge_eval.db"
        artifacts_path = tmp_path / "artifacts"
        port = 18766
        base_url = f"http://127.0.0.1:{port}"

        env = {
            **dict(subprocess.os.environ),
            "FORGE_EVAL_STORAGE_BACKEND": "sqlite",
            "FORGE_EVAL_DB_PATH": str(db_path),
            "FORGE_EVAL_ARTIFACTS_PATH": str(artifacts_path),
            "FORGE_EVAL_TASKS_PATH": str(tasks_path),
            "FORGE_EVAL_RUN_EXECUTION_MODE": "queued",
        }

        server = subprocess.Popen(
            [sys.executable, "-m", "uvicorn", "app.main:app", "--port", str(port)],
            cwd=str(Path(__file__).parent.parent),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            _wait_for_url(f"{base_url}/health")

            # Create 3 runs
            run_ids = []
            for _ in range(3):
                resp = httpx.post(
                    f"{base_url}/runs",
                    json={"task_ids": ["task-pass"], "provider": "mock"},
                    timeout=5,
                )
                assert resp.status_code == 201
                run_ids.append(resp.json()["run_id"])

            # Start worker with short poll interval; let it run for a bit then stop
            worker = subprocess.Popen(
                [
                    sys.executable,
                    "-m",
                    "app.worker",
                    "--poll-interval-seconds",
                    "0.1",
                ],
                cwd=str(Path(__file__).parent.parent),
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            # Wait for all runs to complete (with timeout)
            deadline = time.time() + 15
            while time.time() < deadline:
                runs_resp = httpx.get(f"{base_url}/runs?status=completed", timeout=5)
                completed = [r["run_id"] for r in runs_resp.json()]
                if all(rid in completed for rid in run_ids):
                    break
                time.sleep(0.2)
            else:
                worker.terminate()
                worker.wait(timeout=5)
                raise TimeoutError("Not all runs completed within timeout")

            # Gracefully stop the worker
            worker.terminate()
            try:
                worker.wait(timeout=5)
            except subprocess.TimeoutExpired:
                worker.kill()
                worker.wait(timeout=2)

            # Verify all 3 runs completed with artifacts
            for run_id in run_ids:
                run_resp = httpx.get(f"{base_url}/runs/{run_id}", timeout=5)
                assert run_resp.json()["status"] == "completed"
                artifacts_resp = httpx.get(f"{base_url}/runs/{run_id}/artifacts", timeout=5)
                assert {a["kind"] for a in artifacts_resp.json()} == {"report", "trace"}
        finally:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
