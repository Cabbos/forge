import json
import sqlite3
from collections.abc import Sequence
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Protocol
from uuid import uuid4

from pydantic import TypeAdapter

from app.cases import CaseLoadError, load_cases
from app.metrics import calculate_metrics
from app.models import (
    AgentTrace,
    EvalArtifact,
    EvalProvider,
    EvaluationRun,
    EvaluationTask,
    FailureCategory,
    MetricsSummary,
    QueueStatus,
    RunStatus,
    TrustGateResult,
)
from app.reporting import build_report


class TaskLoadError(RuntimeError):
    pass


class LeaseLostError(RuntimeError):
    """Raised when a worker no longer owns the claimed run attempt."""


class EvalStorage(Protocol):
    def list_tasks(self) -> list[EvaluationTask]: ...
    def get_tasks(self, task_ids: list[str] | None) -> list[EvaluationTask]: ...
    def create_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def save_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def get_run(self, run_id: str) -> EvaluationRun | None: ...
    def list_runs(self, status_filter: str | None = None) -> list[EvaluationRun]: ...
    def queue_status(self) -> QueueStatus: ...
    def claim_pending_run(
        self,
        worker_id: str | None = None,
        lease_duration_seconds: float = 300.0,
    ) -> EvaluationRun | None: ...
    def save_task(
        self,
        run_id: str,
        trace: AgentTrace,
        *,
        worker_id: str,
        lease_token: str,
    ) -> None: ...
    def save_artifact(self, artifact: EvalArtifact) -> EvalArtifact: ...
    def list_artifacts(
        self, run_id: str, *, include_attempts: bool = False
    ) -> list[EvalArtifact]: ...
    def update_run_status(self, run_id: str, status: RunStatus) -> None: ...
    def cancel_run(self, run_id: str) -> EvaluationRun | None: ...
    def heartbeat_run(
        self,
        run_id: str,
        worker_id: str,
        lease_token: str,
        lease_expires_at: datetime,
    ) -> None: ...
    def complete_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun: ...
    def fail_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun: ...
    def retry_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun: ...


class InMemoryStorage:
    """Simple storage for local API use and storage contract tests."""

    def __init__(self, tasks_path: Path | str) -> None:
        self.tasks_path = Path(tasks_path)
        self._tasks = self._load_tasks()
        self._runs: dict[str, EvaluationRun] = {}
        self._artifacts: dict[str, list[EvalArtifact]] = {}

    def _load_tasks(self) -> dict[str, EvaluationTask]:
        try:
            tasks = load_cases(self.tasks_path)
        except CaseLoadError as exc:
            raise TaskLoadError(f"Could not load tasks from {self.tasks_path}") from exc

        return {task.id: task for task in tasks}

    def list_tasks(self) -> list[EvaluationTask]:
        return list(self._tasks.values())

    def get_tasks(self, task_ids: list[str] | None) -> list[EvaluationTask]:
        if task_ids is None:
            return self.list_tasks()

        missing = [task_id for task_id in task_ids if task_id not in self._tasks]
        if missing:
            missing_text = ", ".join(missing)
            raise KeyError(f"Unknown task id(s): {missing_text}")

        return [self._tasks[task_id] for task_id in task_ids]

    def create_run(self, run: EvaluationRun) -> EvaluationRun:
        self._runs[run.run_id] = run
        self._artifacts.setdefault(run.run_id, [])
        return run

    def save_run(self, run: EvaluationRun) -> EvaluationRun:
        self._runs[run.run_id] = run
        self._artifacts.setdefault(run.run_id, [])
        return run

    def get_run(self, run_id: str) -> EvaluationRun | None:
        return self._runs.get(run_id)

    def list_runs(self, status_filter: str | None = None) -> list[EvaluationRun]:
        runs = list(self._runs.values())
        if status_filter:
            runs = [r for r in runs if r.status.value == status_filter]
        return runs

    def queue_status(self) -> QueueStatus:
        counts: dict[str, int] = {}
        oldest_pending_run_id: str | None = None
        oldest_running_run_id: str | None = None
        for run in self._runs.values():
            counts[run.status.value] = counts.get(run.status.value, 0) + 1
            if run.status == RunStatus.PENDING and oldest_pending_run_id is None:
                oldest_pending_run_id = run.run_id
            if run.status == RunStatus.RUNNING and oldest_running_run_id is None:
                oldest_running_run_id = run.run_id
        return QueueStatus(
            counts=counts,
            oldest_pending_run_id=oldest_pending_run_id,
            oldest_running_run_id=oldest_running_run_id,
        )

    def claim_pending_run(
        self,
        worker_id: str | None = None,
        lease_duration_seconds: float = 300.0,
    ) -> EvaluationRun | None:
        now = datetime.now(UTC)
        lease_expires = now + timedelta(seconds=lease_duration_seconds)
        for run in self._runs.values():
            if run.status == RunStatus.PENDING:
                claimed = run.model_copy(
                    update={
                        "status": RunStatus.RUNNING,
                        "worker_id": worker_id,
                        "lease_token": str(uuid4()),
                        "claimed_at": now,
                        "heartbeat_at": None,
                        "lease_expires_at": lease_expires,
                    }
                )
                self._runs[run.run_id] = claimed
                return claimed
            if (
                run.status == RunStatus.RUNNING
                and run.lease_expires_at is not None
                and datetime.now(UTC) > run.lease_expires_at
            ):
                reclaimed = run.model_copy(
                    update={
                        "worker_id": worker_id,
                        "lease_token": str(uuid4()),
                        "claimed_at": now,
                        "heartbeat_at": None,
                        "lease_expires_at": lease_expires,
                    }
                )
                self._runs[run.run_id] = reclaimed
                return reclaimed
        return None

    def save_task(
        self,
        run_id: str,
        trace: AgentTrace,
        *,
        worker_id: str,
        lease_token: str,
    ) -> None:
        run = self._require_active_lease(run_id, worker_id, lease_token)
        traces = replace_trace(run.traces, trace)
        self._runs[run_id] = run.model_copy(
            update={
                "traces": traces,
                "metrics": calculate_metrics(traces),
            }
        )

    def save_artifact(self, artifact: EvalArtifact) -> EvalArtifact:
        artifacts = [
            existing
            for existing in self._artifacts.setdefault(artifact.run_id, [])
            if existing.id != artifact.id
        ]
        artifacts.append(artifact)
        self._artifacts[artifact.run_id] = artifacts
        return artifact

    def list_artifacts(
        self,
        run_id: str,
        *,
        include_attempts: bool = False,
    ) -> list[EvalArtifact]:
        artifacts = list(self._artifacts.get(run_id, []))
        if include_attempts:
            return artifacts
        return [artifact for artifact in artifacts if not artifact.kind.endswith("_attempt")]

    def update_run_status(self, run_id: str, status: RunStatus) -> None:
        run = self._require_run(run_id)
        self._runs[run_id] = run.model_copy(update={"status": status})

    def cancel_run(self, run_id: str) -> EvaluationRun | None:
        run = self.get_run(run_id)
        if run is None:
            return None
        if run.status in {RunStatus.COMPLETED, RunStatus.FAILED, RunStatus.CANCELLED}:
            raise RunAlreadyTerminalError(f"Run {run_id} is already {run.status.value}")
        cancelled = run.model_copy(update={"status": RunStatus.CANCELLED})
        self._runs[run_id] = cancelled
        return cancelled

    def heartbeat_run(
        self,
        run_id: str,
        worker_id: str,
        lease_token: str,
        lease_expires_at: datetime,
    ) -> None:
        run = self._require_active_lease(run_id, worker_id, lease_token)
        self._runs[run_id] = run.model_copy(
            update={
                "worker_id": worker_id,
                "heartbeat_at": datetime.now(UTC),
                "lease_expires_at": lease_expires_at,
            }
        )

    def complete_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.COMPLETED, worker_id, lease_token)

    def fail_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.FAILED, worker_id, lease_token)

    def retry_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.PENDING, worker_id, lease_token)

    def _finalize_run(
        self,
        run: EvaluationRun,
        target_status: RunStatus,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        current = self._runs.get(run.run_id)
        if current is None:
            raise KeyError(f"Unknown run id: {run.run_id}")
        if (
            current.status == RunStatus.CANCELLED
            and current.worker_id == worker_id
            and current.lease_token == lease_token
        ):
            return current
        self._require_active_lease(run.run_id, worker_id, lease_token)
        update: dict[str, object] = {
            "status": target_status,
            "worker_id": worker_id,
            "lease_token": lease_token,
        }
        if target_status == RunStatus.PENDING:
            update.update(
                {
                    "worker_id": None,
                    "lease_token": None,
                    "claimed_at": None,
                    "heartbeat_at": None,
                    "lease_expires_at": None,
                }
            )
        finalized = run.model_copy(update=update)
        self._runs[run.run_id] = finalized
        self._artifacts.setdefault(run.run_id, [])
        return finalized

    def _require_active_lease(
        self,
        run_id: str,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        run = self._require_run(run_id)
        now = datetime.now(UTC)
        active = (
            run.status == RunStatus.RUNNING
            and run.worker_id == worker_id
            and run.lease_token == lease_token
            and run.lease_expires_at is not None
            and run.lease_expires_at >= now
        )
        if not active:
            raise LeaseLostError(f"Worker {worker_id} lost lease for run {run_id}")
        return run

    def force_lease_expiry_for_test(self, run_id: str, lease_expires_at: datetime) -> None:
        run = self._require_run(run_id)
        self._runs[run_id] = run.model_copy(update={"lease_expires_at": lease_expires_at})

    def _require_run(self, run_id: str) -> EvaluationRun:
        run = self.get_run(run_id)
        if run is None:
            raise KeyError(f"Unknown run id: {run_id}")
        return run


class SQLiteStorage:
    """SQLite-backed storage that keeps large trace/report JSON in artifact files."""

    def __init__(
        self,
        *,
        tasks_path: Path | str,
        db_path: Path | str,
        artifacts_path: Path | str,
    ) -> None:
        self.tasks_path = Path(tasks_path)
        self.db_path = Path(db_path)
        self.artifacts_path = Path(artifacts_path)
        self._tasks = self._load_tasks()
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self.artifacts_path.mkdir(parents=True, exist_ok=True)
        self._init_schema()

    def _load_tasks(self) -> dict[str, EvaluationTask]:
        try:
            tasks = load_cases(self.tasks_path)
        except CaseLoadError as exc:
            raise TaskLoadError(f"Could not load tasks from {self.tasks_path}") from exc

        return {task.id: task for task in tasks}

    def list_tasks(self) -> list[EvaluationTask]:
        return list(self._tasks.values())

    def get_tasks(self, task_ids: list[str] | None) -> list[EvaluationTask]:
        if task_ids is None:
            return self.list_tasks()

        missing = [task_id for task_id in task_ids if task_id not in self._tasks]
        if missing:
            missing_text = ", ".join(missing)
            raise KeyError(f"Unknown task id(s): {missing_text}")

        return [self._tasks[task_id] for task_id in task_ids]

    def create_run(self, run: EvaluationRun) -> EvaluationRun:
        self._upsert_run(run)
        return run

    def save_run(self, run: EvaluationRun) -> EvaluationRun:
        self._write_run_artifacts(run)
        self._upsert_run(run)
        with self._connect() as connection:
            connection.execute("DELETE FROM eval_run_tasks WHERE run_id = ?", (run.run_id,))
            for trace in run.traces:
                self._upsert_task(connection, run.run_id, trace)
        return run

    def get_run(self, run_id: str) -> EvaluationRun | None:
        with self._connect() as connection:
            row = connection.execute(
                "SELECT * FROM eval_runs WHERE id = ?",
                (run_id,),
            ).fetchone()
        if row is None:
            return None

        def _row_val(key: str, default=None):
            try:
                return row[key]
            except IndexError:
                return default

        run_status = RunStatus(row["status"])
        lease_token = _row_val("lease_token")
        attempt_token = (
            lease_token if run_status in {RunStatus.RUNNING, RunStatus.CANCELLED} else None
        )
        traces = self._read_trace_artifact(run_id, attempt_token=attempt_token)
        return EvaluationRun(
            run_id=row["id"],
            status=run_status,
            provider=(
                EvalProvider(provider_value)
                if (provider_value := _row_val("provider")) is not None
                else None
            ),
            model=_row_val("model"),
            case_source=_row_val("case_source"),
            requested_task_ids=json.loads(row["requested_task_ids_json"]),
            traces=traces,
            metrics=MetricsSummary.model_validate(json.loads(row["metrics_json"])),
            trust_result=(
                TrustGateResult.model_validate_json(trust_payload)
                if (trust_payload := _row_val("trust_result_json"))
                else TrustGateResult()
            ),
            started_at=datetime.fromisoformat(row["started_at"]),
            ended_at=datetime.fromisoformat(row["ended_at"]),
            duration_ms=row["duration_ms"],
            retry_count=_row_val("retry_count", 0) or 0,
            max_retries=_row_val("max_retries", 0) or 0,
            failure_reason=_row_val("failure_reason"),
            failure_category=FailureCategory(_row_val("failure_category") or "none"),
            worker_id=_row_val("worker_id"),
            lease_token=lease_token,
            claimed_at=_parse_datetime(_row_val("claimed_at")),
            heartbeat_at=_parse_datetime(_row_val("heartbeat_at")),
            lease_expires_at=_parse_datetime(_row_val("lease_expires_at")),
        )

    def list_runs(self, status_filter: str | None = None) -> list[EvaluationRun]:
        with self._connect() as connection:
            if status_filter:
                rows = connection.execute(
                    "SELECT id FROM eval_runs WHERE status = ? ORDER BY created_at ASC, id ASC",
                    (status_filter,),
                ).fetchall()
            else:
                rows = connection.execute(
                    "SELECT id FROM eval_runs ORDER BY created_at ASC, id ASC"
                ).fetchall()
        return [run for row in rows if (run := self.get_run(row["id"])) is not None]

    def queue_status(self) -> QueueStatus:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT status, COUNT(*) AS count FROM eval_runs GROUP BY status"
            ).fetchall()
            counts = {row["status"]: row["count"] for row in rows}
            oldest_pending_run_id = self._oldest_run_id_by_status(connection, RunStatus.PENDING)
            oldest_running_run_id = self._oldest_run_id_by_status(connection, RunStatus.RUNNING)
        return QueueStatus(
            counts=counts,
            oldest_pending_run_id=oldest_pending_run_id,
            oldest_running_run_id=oldest_running_run_id,
        )

    def save_task(
        self,
        run_id: str,
        trace: AgentTrace,
        *,
        worker_id: str,
        lease_token: str,
    ) -> None:
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            run = self._require_active_lease_connection(
                connection,
                run_id,
                worker_id,
                lease_token,
            )
            traces = replace_trace(run.traces, trace)
            updated = run.model_copy(
                update={
                    "traces": traces,
                    "metrics": calculate_metrics(traces),
                }
            )
            self._write_attempt_artifacts(updated, lease_token, connection)
            self._upsert_run_connection(connection, updated)
            self._upsert_task(connection, run_id, trace)
            connection.execute("COMMIT")

    def save_artifact(self, artifact: EvalArtifact) -> EvalArtifact:
        with self._connect() as connection:
            self._upsert_artifact(connection, artifact)
        return artifact

    def list_artifacts(
        self,
        run_id: str,
        *,
        include_attempts: bool = False,
    ) -> list[EvalArtifact]:
        with self._connect() as connection:
            if include_attempts:
                rows = connection.execute(
                    "SELECT * FROM eval_artifacts WHERE run_id = ? ORDER BY created_at ASC, id ASC",
                    (run_id,),
                ).fetchall()
            else:
                rows = connection.execute(
                    "SELECT * FROM eval_artifacts "
                    "WHERE run_id = ? AND kind NOT LIKE '%_attempt' "
                    "ORDER BY created_at ASC, id ASC",
                    (run_id,),
                ).fetchall()
        return [artifact_from_row(row) for row in rows]

    def update_run_status(self, run_id: str, status: RunStatus) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE eval_runs
                SET status = ?, updated_at = ?
                WHERE id = ?
                """,
                (status.value, utc_now_iso(), run_id),
            )

    def _init_schema(self) -> None:
        with self._connect() as connection:
            connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS eval_runs (
                    id TEXT PRIMARY KEY,
                    status TEXT NOT NULL,
                    requested_task_ids_json TEXT NOT NULL,
                    provider TEXT,
                    model TEXT,
                    case_source TEXT,
                    trust_result_json TEXT,
                    lease_token TEXT,
                    success_rate REAL NOT NULL DEFAULT 0,
                    verification_pass_rate REAL NOT NULL DEFAULT 0,
                    scope_violation_rate REAL NOT NULL DEFAULT 0,
                    failure_categories_json TEXT NOT NULL DEFAULT '{}',
                    metrics_json TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    ended_at TEXT NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS eval_run_tasks (
                    run_id TEXT NOT NULL,
                    task_id TEXT NOT NULL,
                    status TEXT NOT NULL,
                    passed INTEGER NOT NULL,
                    verification_passed INTEGER,
                    duration_ms INTEGER NOT NULL,
                    model_rounds INTEGER NOT NULL,
                    confirm_requests INTEGER NOT NULL,
                    changed_files_json TEXT NOT NULL,
                    scope_violations_json TEXT NOT NULL,
                    failure_category TEXT NOT NULL,
                    failure_reason TEXT,
                    PRIMARY KEY (run_id, task_id),
                    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS eval_artifacts (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    path TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    attempt_token TEXT,
                    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS eval_experiments (
                    id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    dataset_fingerprint TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    git_commit TEXT,
                    command TEXT,
                    environment_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE
                );
                """
            )
            ensure_column(connection, "eval_runs", "provider", "TEXT")
            ensure_column(connection, "eval_runs", "model", "TEXT")
            ensure_column(connection, "eval_runs", "case_source", "TEXT")
            ensure_column(connection, "eval_runs", "trust_result_json", "TEXT")
            ensure_column(connection, "eval_runs", "lease_token", "TEXT")
            ensure_column(connection, "eval_runs", "retry_count", "INTEGER NOT NULL DEFAULT 0")
            ensure_column(connection, "eval_runs", "max_retries", "INTEGER NOT NULL DEFAULT 0")
            ensure_column(connection, "eval_runs", "failure_reason", "TEXT")
            ensure_column(connection, "eval_runs", "failure_category", "TEXT")
            ensure_column(connection, "eval_runs", "worker_id", "TEXT")
            ensure_column(connection, "eval_runs", "claimed_at", "TEXT")
            ensure_column(connection, "eval_runs", "heartbeat_at", "TEXT")
            ensure_column(connection, "eval_runs", "lease_expires_at", "TEXT")
            ensure_column(connection, "eval_artifacts", "attempt_token", "TEXT")

    def claim_pending_run(
        self,
        worker_id: str | None = None,
        lease_duration_seconds: float = 300.0,
    ) -> EvaluationRun | None:
        now = datetime.now(UTC)
        lease_expires = now + timedelta(seconds=lease_duration_seconds)
        lease_token = str(uuid4())
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            # First try to claim a pending run
            row = connection.execute(
                """
                SELECT id
                FROM eval_runs
                WHERE status = ?
                ORDER BY created_at ASC, id ASC
                LIMIT 1
                """,
                (RunStatus.PENDING.value,),
            ).fetchone()
            # Then try to reclaim a stale running run
            if row is None:
                row = connection.execute(
                    """
                    SELECT id
                    FROM eval_runs
                    WHERE status = ?
                        AND (lease_expires_at IS NULL OR lease_expires_at < ?)
                    ORDER BY created_at ASC, id ASC
                    LIMIT 1
                    """,
                    (RunStatus.RUNNING.value, now.isoformat()),
                ).fetchone()
            if row is None:
                return None
            connection.execute(
                """
                UPDATE eval_runs
                SET status = ?, updated_at = ?, worker_id = ?, lease_token = ?,
                    claimed_at = ?, heartbeat_at = NULL, lease_expires_at = ?
                WHERE id = ?
                """,
                (
                    RunStatus.RUNNING.value,
                    utc_now_iso(),
                    worker_id,
                    lease_token,
                    now.isoformat(),
                    lease_expires.isoformat(),
                    row["id"],
                ),
            )
            run_id = row["id"]
        return self.get_run(run_id)

    def cancel_run(self, run_id: str) -> EvaluationRun | None:
        with self._connect() as connection:
            row = connection.execute(
                "SELECT status FROM eval_runs WHERE id = ?", (run_id,)
            ).fetchone()
            if row is None:
                return None
            if row["status"] in {
                RunStatus.COMPLETED.value,
                RunStatus.FAILED.value,
                RunStatus.CANCELLED.value,
            }:
                raise RunAlreadyTerminalError(f"Run {run_id} is already {row['status']}")
            connection.execute(
                """
                UPDATE eval_runs
                SET status = ?, updated_at = ?
                WHERE id = ?
                """,
                (RunStatus.CANCELLED.value, utc_now_iso(), run_id),
            )
        return self.get_run(run_id)

    def heartbeat_run(
        self,
        run_id: str,
        worker_id: str,
        lease_token: str,
        lease_expires_at: datetime,
    ) -> None:
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            self._require_active_lease_connection(
                connection,
                run_id,
                worker_id,
                lease_token,
            )
            cursor = connection.execute(
                """
                UPDATE eval_runs
                SET heartbeat_at = ?, lease_expires_at = ?, updated_at = ?
                WHERE id = ? AND status = ? AND worker_id = ? AND lease_token = ?
                    AND lease_expires_at >= ?
                """,
                (
                    datetime.now(UTC).isoformat(),
                    lease_expires_at.isoformat(),
                    utc_now_iso(),
                    run_id,
                    RunStatus.RUNNING.value,
                    worker_id,
                    lease_token,
                    datetime.now(UTC).isoformat(),
                ),
            )
            if cursor.rowcount != 1:
                raise LeaseLostError(f"Worker {worker_id} lost lease for run {run_id}")
            connection.execute("COMMIT")

    def complete_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.COMPLETED, worker_id, lease_token)

    def fail_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.FAILED, worker_id, lease_token)

    def retry_run(
        self,
        run: EvaluationRun,
        *,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.PENDING, worker_id, lease_token)

    def _finalize_run(
        self,
        run: EvaluationRun,
        target_status: RunStatus,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT status, worker_id, lease_token FROM eval_runs WHERE id = ?",
                (run.run_id,),
            ).fetchone()
            if row is None:
                raise KeyError(f"Unknown run id: {run.run_id}")
            current_status = RunStatus(row["status"])
            if current_status == RunStatus.CANCELLED:
                same_attempt = row["worker_id"] == worker_id and row["lease_token"] == lease_token
                if not same_attempt:
                    raise LeaseLostError(f"Worker {worker_id} lost lease for run {run.run_id}")
                connection.execute("ROLLBACK")
                stored_run = self.get_run(run.run_id)
                if stored_run is None:
                    raise KeyError(f"Unknown run id after rollback: {run.run_id}")
                return stored_run
            self._require_active_lease_connection(
                connection,
                run.run_id,
                worker_id,
                lease_token,
            )
            update: dict[str, object] = {
                "status": target_status,
                "worker_id": worker_id,
                "lease_token": lease_token,
            }
            if target_status == RunStatus.PENDING:
                update.update(
                    {
                        "worker_id": None,
                        "lease_token": None,
                        "claimed_at": None,
                        "heartbeat_at": None,
                        "lease_expires_at": None,
                    }
                )
            finalized = run.model_copy(update=update)
            if target_status != RunStatus.PENDING:
                self._write_run_artifacts(
                    finalized,
                    connection,
                    attempt_token=lease_token,
                )
            self._upsert_run_connection(connection, finalized)
            connection.execute("DELETE FROM eval_run_tasks WHERE run_id = ?", (run.run_id,))
            if target_status != RunStatus.PENDING:
                for trace in finalized.traces:
                    self._upsert_task(connection, finalized.run_id, trace)
            connection.execute("COMMIT")
        stored_run = self.get_run(run.run_id)
        if stored_run is None:
            raise KeyError(f"Unknown run id after finalize: {run.run_id}")
        return stored_run

    def _require_active_lease_connection(
        self,
        connection: sqlite3.Connection,
        run_id: str,
        worker_id: str,
        lease_token: str,
    ) -> EvaluationRun:
        row = connection.execute(
            """
            SELECT status, worker_id, lease_token, lease_expires_at
            FROM eval_runs
            WHERE id = ?
            """,
            (run_id,),
        ).fetchone()
        if row is None:
            raise KeyError(f"Unknown run id: {run_id}")
        lease_expires_at = _parse_datetime(row["lease_expires_at"])
        active = (
            row["status"] == RunStatus.RUNNING.value
            and row["worker_id"] == worker_id
            and row["lease_token"] == lease_token
            and lease_expires_at is not None
            and lease_expires_at >= datetime.now(UTC)
        )
        if not active:
            raise LeaseLostError(f"Worker {worker_id} lost lease for run {run_id}")
        run = self.get_run(run_id)
        if run is None:
            raise KeyError(f"Unknown run id after lease check: {run_id}")
        return run

    def force_lease_expiry_for_test(self, run_id: str, lease_expires_at: datetime) -> None:
        with self._connect() as connection:
            cursor = connection.execute(
                "UPDATE eval_runs SET lease_expires_at = ?, updated_at = ? WHERE id = ?",
                (lease_expires_at.isoformat(), utc_now_iso(), run_id),
            )
            if cursor.rowcount != 1:
                raise KeyError(f"Unknown run id: {run_id}")

    def _upsert_run(self, run: EvaluationRun) -> None:
        with self._connect() as connection:
            self._upsert_run_connection(connection, run)

    def _upsert_run_connection(self, connection: sqlite3.Connection, run: EvaluationRun) -> None:
        report = build_report(run.traces)
        now = utc_now_iso()
        existing = connection.execute(
            "SELECT created_at FROM eval_runs WHERE id = ?",
            (run.run_id,),
        ).fetchone()
        created_at = existing["created_at"] if existing is not None else now
        connection.execute(
            """
            INSERT INTO eval_runs (
                id, status, requested_task_ids_json, provider, model, success_rate,
                case_source, verification_pass_rate, scope_violation_rate,
                failure_categories_json, metrics_json, trust_result_json,
                started_at, ended_at, duration_ms,
                retry_count, max_retries, failure_reason, failure_category,
                worker_id, lease_token, claimed_at, heartbeat_at, lease_expires_at,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                requested_task_ids_json = excluded.requested_task_ids_json,
                provider = excluded.provider,
                model = excluded.model,
                case_source = excluded.case_source,
                success_rate = excluded.success_rate,
                verification_pass_rate = excluded.verification_pass_rate,
                scope_violation_rate = excluded.scope_violation_rate,
                failure_categories_json = excluded.failure_categories_json,
                metrics_json = excluded.metrics_json,
                trust_result_json = excluded.trust_result_json,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                duration_ms = excluded.duration_ms,
                retry_count = excluded.retry_count,
                max_retries = excluded.max_retries,
                failure_reason = excluded.failure_reason,
                failure_category = excluded.failure_category,
                worker_id = excluded.worker_id,
                lease_token = excluded.lease_token,
                claimed_at = excluded.claimed_at,
                heartbeat_at = excluded.heartbeat_at,
                lease_expires_at = excluded.lease_expires_at,
                updated_at = excluded.updated_at
            """,
            (
                run.run_id,
                run.status.value,
                json.dumps(run.requested_task_ids),
                run.provider.value if run.provider is not None else None,
                run.model,
                report.success_rate,
                run.case_source,
                report.verification_pass_rate,
                report.scope_violation_rate,
                json.dumps(report.failure_categories),
                run.metrics.model_dump_json(),
                run.trust_result.model_dump_json(),
                run.started_at.isoformat(),
                run.ended_at.isoformat(),
                run.duration_ms,
                run.retry_count,
                run.max_retries,
                run.failure_reason,
                run.failure_category.value,
                run.worker_id,
                run.lease_token,
                run.claimed_at.isoformat() if run.claimed_at is not None else None,
                run.heartbeat_at.isoformat() if run.heartbeat_at is not None else None,
                run.lease_expires_at.isoformat() if run.lease_expires_at is not None else None,
                created_at,
                now,
            ),
        )

    def _write_run_artifacts(
        self,
        run: EvaluationRun,
        connection: sqlite3.Connection | None = None,
        *,
        attempt_token: str | None = None,
    ) -> None:
        run_artifacts_path = self.artifacts_path / run.run_id
        run_artifacts_path.mkdir(parents=True, exist_ok=True)
        trace_path = run_artifacts_path / "trace.json"
        report_path = run_artifacts_path / "report.json"
        trajectory_paths = write_trajectory_artifacts(run, run_artifacts_path)
        trace_path.write_text(
            json.dumps([trace.model_dump(mode="json") for trace in run.traces], indent=2),
            encoding="utf-8",
        )
        report_path.write_text(
            build_report(run.traces)
            .model_copy(update={"trust_result": run.trust_result})
            .model_dump_json(indent=2),
            encoding="utf-8",
        )
        if connection is not None:
            self._upsert_artifact(
                connection,
                artifact_for_path(
                    run.run_id,
                    "trace",
                    trace_path,
                    attempt_token=attempt_token,
                ),
            )
            self._upsert_artifact(
                connection,
                artifact_for_path(
                    run.run_id,
                    "report",
                    report_path,
                    attempt_token=attempt_token,
                ),
            )
            for trace, trajectory_path in zip(run.traces, trajectory_paths, strict=False):
                self._upsert_artifact(
                    connection,
                    trajectory_artifact_for_path(
                        run.run_id,
                        trace.task_id,
                        trajectory_path,
                        attempt_token=attempt_token,
                    ),
                )
        else:
            with self._connect() as connection:
                self._upsert_artifact(
                    connection,
                    artifact_for_path(
                        run.run_id,
                        "trace",
                        trace_path,
                        attempt_token=attempt_token,
                    ),
                )
                self._upsert_artifact(
                    connection,
                    artifact_for_path(
                        run.run_id,
                        "report",
                        report_path,
                        attempt_token=attempt_token,
                    ),
                )
                for trace, trajectory_path in zip(run.traces, trajectory_paths, strict=False):
                    self._upsert_artifact(
                        connection,
                        trajectory_artifact_for_path(
                            run.run_id,
                            trace.task_id,
                            trajectory_path,
                            attempt_token=attempt_token,
                        ),
                    )

    def _write_attempt_artifacts(
        self,
        run: EvaluationRun,
        lease_token: str,
        connection: sqlite3.Connection,
    ) -> None:
        attempt_path = self.artifacts_path / run.run_id / "attempts" / lease_token
        attempt_path.mkdir(parents=True, exist_ok=True)
        trace_path = attempt_path / "trace.json"
        report_path = attempt_path / "report.json"
        trajectory_paths = write_trajectory_artifacts(run, attempt_path)
        trace_path.write_text(
            json.dumps([trace.model_dump(mode="json") for trace in run.traces], indent=2),
            encoding="utf-8",
        )
        report_path.write_text(
            build_report(run.traces)
            .model_copy(update={"trust_result": run.trust_result})
            .model_dump_json(indent=2),
            encoding="utf-8",
        )
        self._upsert_artifact(
            connection,
            artifact_for_path(
                run.run_id,
                "trace_attempt",
                trace_path,
                attempt_token=lease_token,
            ),
        )
        self._upsert_artifact(
            connection,
            artifact_for_path(
                run.run_id,
                "report_attempt",
                report_path,
                attempt_token=lease_token,
            ),
        )
        for trace, trajectory_path in zip(run.traces, trajectory_paths, strict=False):
            self._upsert_artifact(
                connection,
                trajectory_artifact_for_path(
                    run.run_id,
                    trace.task_id,
                    trajectory_path,
                    kind="trajectory_attempt",
                    attempt_token=lease_token,
                ),
            )

    def _read_trace_artifact(
        self,
        run_id: str,
        *,
        attempt_token: str | None = None,
    ) -> list[AgentTrace]:
        artifacts = self.list_artifacts(run_id, include_attempts=attempt_token is not None)
        trace_artifacts = [
            artifact
            for artifact in artifacts
            if (
                artifact.kind == ("trace_attempt" if attempt_token is not None else "trace")
                and (attempt_token is None or artifact.attempt_token == attempt_token)
            )
        ]
        if not trace_artifacts:
            return []
        trace_path = Path(trace_artifacts[-1].path)
        if not trace_path.exists():
            return []
        payload = json.loads(trace_path.read_text(encoding="utf-8"))
        return TypeAdapter(list[AgentTrace]).validate_python(payload)

    def _upsert_task(
        self,
        connection: sqlite3.Connection,
        run_id: str,
        trace: AgentTrace,
    ) -> None:
        report = build_report([trace])
        summary = report.tasks[0]
        connection.execute(
            """
            INSERT INTO eval_run_tasks (
                run_id, task_id, status, passed, verification_passed, duration_ms,
                model_rounds, confirm_requests, changed_files_json, scope_violations_json,
                failure_category, failure_reason
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(run_id, task_id) DO UPDATE SET
                status = excluded.status,
                passed = excluded.passed,
                verification_passed = excluded.verification_passed,
                duration_ms = excluded.duration_ms,
                model_rounds = excluded.model_rounds,
                confirm_requests = excluded.confirm_requests,
                changed_files_json = excluded.changed_files_json,
                scope_violations_json = excluded.scope_violations_json,
                failure_category = excluded.failure_category,
                failure_reason = excluded.failure_reason
            """,
            (
                run_id,
                trace.task_id,
                "completed" if summary.passed else "failed",
                int(summary.passed),
                optional_bool_to_int(summary.verification_passed),
                summary.duration_ms,
                summary.model_rounds,
                summary.confirm_requests,
                json.dumps(summary.changed_files),
                json.dumps(summary.scope_violations),
                summary.failure_category.value,
                summary.failure_reason,
            ),
        )

    def _upsert_artifact(
        self,
        connection: sqlite3.Connection,
        artifact: EvalArtifact,
    ) -> None:
        connection.execute(
            """
            INSERT INTO eval_artifacts (
                id, run_id, kind, path, size_bytes, created_at, attempt_token
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                run_id = excluded.run_id,
                kind = excluded.kind,
                path = excluded.path,
                size_bytes = excluded.size_bytes,
                created_at = excluded.created_at,
                attempt_token = excluded.attempt_token
            """,
            (
                artifact.id,
                artifact.run_id,
                artifact.kind,
                artifact.path,
                artifact.size_bytes,
                artifact.created_at.isoformat(),
                artifact.attempt_token,
            ),
        )

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.db_path)
        connection.row_factory = sqlite3.Row
        return connection

    def _oldest_run_id_by_status(
        self,
        connection: sqlite3.Connection,
        status: RunStatus,
    ) -> str | None:
        row = connection.execute(
            """
            SELECT id
            FROM eval_runs
            WHERE status = ?
            ORDER BY created_at ASC, id ASC
            LIMIT 1
            """,
            (status.value,),
        ).fetchone()
        return row["id"] if row is not None else None


def replace_trace(traces: Sequence[AgentTrace], trace: AgentTrace) -> list[AgentTrace]:
    filtered = [existing for existing in traces if existing.task_id != trace.task_id]
    return [*filtered, trace]


def optional_bool_to_int(value: bool | None) -> int | None:
    if value is None:
        return None
    return int(value)


def artifact_for_path(
    run_id: str,
    kind: str,
    path: Path,
    *,
    attempt_token: str | None = None,
) -> EvalArtifact:
    attempt_suffix = (
        f":{safe_artifact_id_part(attempt_token)}"
        if attempt_token is not None and kind.endswith("_attempt")
        else ""
    )
    return EvalArtifact(
        id=f"{run_id}:{kind}{attempt_suffix}",
        run_id=run_id,
        kind=kind,
        path=str(path),
        size_bytes=path.stat().st_size,
        created_at=datetime.now(UTC),
        attempt_token=attempt_token,
    )


def trajectory_artifact_for_path(
    run_id: str,
    task_id: str,
    path: Path,
    *,
    kind: str = "trajectory",
    attempt_token: str | None = None,
) -> EvalArtifact:
    artifact = artifact_for_path(
        run_id,
        kind,
        path,
        attempt_token=attempt_token,
    )
    token_suffix = f":{safe_artifact_id_part(attempt_token)}" if attempt_token is not None else ""
    return artifact.model_copy(
        update={"id": f"{run_id}:{kind}{token_suffix}:{safe_artifact_id_part(task_id)}"}
    )


def write_trajectory_artifacts(run: EvaluationRun, run_artifacts_path: Path) -> list[Path]:
    paths: list[Path] = []
    for trace in run.traces:
        trajectory_path = (
            run_artifacts_path / f"{safe_artifact_id_part(trace.task_id)}.trajectory.json"
        )
        trajectory_path.write_text(
            trace.model_copy(update={"trajectory_path": str(trajectory_path)}).model_dump_json(
                indent=2
            ),
            encoding="utf-8",
        )
        paths.append(trajectory_path)
    return paths


def safe_artifact_id_part(value: str) -> str:
    return value.replace("/", "_").replace("\\", "_")


def artifact_from_row(row: sqlite3.Row) -> EvalArtifact:
    return EvalArtifact(
        id=row["id"],
        run_id=row["run_id"],
        kind=row["kind"],
        path=row["path"],
        size_bytes=row["size_bytes"],
        created_at=datetime.fromisoformat(row["created_at"]),
        attempt_token=row["attempt_token"],
    )


def utc_now_iso() -> str:
    return datetime.now(UTC).isoformat()


class RunAlreadyTerminalError(RuntimeError):
    """Raised when attempting to cancel a run that is already in a terminal state."""

    pass


def _parse_datetime(value: str | None) -> datetime | None:
    if value is None:
        return None
    return datetime.fromisoformat(value)


def ensure_column(
    connection: sqlite3.Connection,
    table_name: str,
    column_name: str,
    column_type: str,
) -> None:
    columns = {
        row["name"] for row in connection.execute(f"PRAGMA table_info({table_name})").fetchall()
    }
    if column_name not in columns:
        connection.execute(f"ALTER TABLE {table_name} ADD COLUMN {column_name} {column_type}")
