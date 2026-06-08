import json
import sqlite3
from collections.abc import Sequence
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Protocol

from pydantic import TypeAdapter

from app.cases import CaseLoadError, load_cases
from app.metrics import calculate_metrics
from app.models import (
    AgentTrace,
    EvalArtifact,
    EvaluationRun,
    EvaluationTask,
    FailureCategory,
    MetricsSummary,
    RunStatus,
)
from app.reporting import build_report


class TaskLoadError(RuntimeError):
    pass


class EvalStorage(Protocol):
    def list_tasks(self) -> list[EvaluationTask]: ...
    def get_tasks(self, task_ids: list[str] | None) -> list[EvaluationTask]: ...
    def create_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def save_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def get_run(self, run_id: str) -> EvaluationRun | None: ...
    def list_runs(self) -> list[EvaluationRun]: ...
    def claim_pending_run(self, worker_id: str | None = None) -> EvaluationRun | None: ...
    def save_task(self, run_id: str, trace: AgentTrace) -> None: ...
    def save_artifact(self, artifact: EvalArtifact) -> EvalArtifact: ...
    def list_artifacts(self, run_id: str) -> list[EvalArtifact]: ...
    def update_run_status(self, run_id: str, status: RunStatus) -> None: ...
    def cancel_run(self, run_id: str) -> EvaluationRun | None: ...
    def heartbeat_run(self, run_id: str, worker_id: str, lease_expires_at: datetime) -> None: ...
    def complete_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def fail_run(self, run: EvaluationRun) -> EvaluationRun: ...
    def retry_run(self, run: EvaluationRun) -> EvaluationRun: ...


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

    def list_runs(self) -> list[EvaluationRun]:
        return list(self._runs.values())

    def claim_pending_run(self, worker_id: str | None = None) -> EvaluationRun | None:
        now = datetime.now(UTC)
        lease_expires = now + timedelta(seconds=300)
        for run in self._runs.values():
            if run.status == RunStatus.PENDING:
                claimed = run.model_copy(
                    update={
                        "status": RunStatus.RUNNING,
                        "worker_id": worker_id,
                        "claimed_at": now,
                        "lease_expires_at": lease_expires,
                    }
                )
                self._runs[run.run_id] = claimed
                return claimed
            if run.status == RunStatus.RUNNING and run.lease_expires_at is not None:
                if datetime.now(UTC) > run.lease_expires_at:
                    reclaimed = run.model_copy(
                        update={
                            "worker_id": worker_id,
                            "claimed_at": now,
                            "lease_expires_at": lease_expires,
                        }
                    )
                    self._runs[run.run_id] = reclaimed
                    return reclaimed
        return None

    def save_task(self, run_id: str, trace: AgentTrace) -> None:
        run = self._require_run(run_id)
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

    def list_artifacts(self, run_id: str) -> list[EvalArtifact]:
        return list(self._artifacts.get(run_id, []))

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

    def heartbeat_run(self, run_id: str, worker_id: str, lease_expires_at: datetime) -> None:
        run = self._require_run(run_id)
        self._runs[run_id] = run.model_copy(
            update={
                "worker_id": worker_id,
                "heartbeat_at": datetime.now(UTC),
                "lease_expires_at": lease_expires_at,
            }
        )

    def complete_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.COMPLETED)

    def fail_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.FAILED)

    def retry_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.PENDING)

    def _finalize_run(self, run: EvaluationRun, target_status: RunStatus) -> EvaluationRun:
        current = self._runs.get(run.run_id)
        if current is None:
            raise KeyError(f"Unknown run id: {run.run_id}")
        if current.status != RunStatus.RUNNING:
            return current
        finalized = run.model_copy(update={"status": target_status})
        self._runs[run.run_id] = finalized
        self._artifacts.setdefault(run.run_id, [])
        return finalized

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

        traces = self._read_trace_artifact(run_id)

        def _row_val(key: str, default=None):
            try:
                return row[key]
            except IndexError:
                return default

        return EvaluationRun(
            run_id=row["id"],
            status=RunStatus(row["status"]),
            requested_task_ids=json.loads(row["requested_task_ids_json"]),
            traces=traces,
            metrics=MetricsSummary.model_validate(json.loads(row["metrics_json"])),
            started_at=datetime.fromisoformat(row["started_at"]),
            ended_at=datetime.fromisoformat(row["ended_at"]),
            duration_ms=row["duration_ms"],
            retry_count=_row_val("retry_count", 0) or 0,
            max_retries=_row_val("max_retries", 0) or 0,
            failure_reason=_row_val("failure_reason"),
            failure_category=FailureCategory(_row_val("failure_category") or "none"),
            worker_id=_row_val("worker_id"),
            claimed_at=_parse_datetime(_row_val("claimed_at")),
            heartbeat_at=_parse_datetime(_row_val("heartbeat_at")),
            lease_expires_at=_parse_datetime(_row_val("lease_expires_at")),
        )

    def list_runs(self) -> list[EvaluationRun]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT id FROM eval_runs ORDER BY created_at ASC, id ASC"
            ).fetchall()
        return [run for row in rows if (run := self.get_run(row["id"])) is not None]

    def save_task(self, run_id: str, trace: AgentTrace) -> None:
        run = self.get_run(run_id)
        if run is None:
            raise KeyError(f"Unknown run id: {run_id}")
        traces = replace_trace(run.traces, trace)
        self.save_run(
            run.model_copy(
                update={
                    "traces": traces,
                    "metrics": calculate_metrics(traces),
                }
            )
        )

    def save_artifact(self, artifact: EvalArtifact) -> EvalArtifact:
        with self._connect() as connection:
            self._upsert_artifact(connection, artifact)
        return artifact

    def list_artifacts(self, run_id: str) -> list[EvalArtifact]:
        with self._connect() as connection:
            rows = connection.execute(
                "SELECT * FROM eval_artifacts WHERE run_id = ? ORDER BY created_at ASC, id ASC",
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
                    FOREIGN KEY (run_id) REFERENCES eval_runs(id) ON DELETE CASCADE
                );
                """
            )
            ensure_column(connection, "eval_runs", "case_source", "TEXT")
            ensure_column(connection, "eval_runs", "retry_count", "INTEGER NOT NULL DEFAULT 0")
            ensure_column(connection, "eval_runs", "max_retries", "INTEGER NOT NULL DEFAULT 0")
            ensure_column(connection, "eval_runs", "failure_reason", "TEXT")
            ensure_column(connection, "eval_runs", "failure_category", "TEXT")
            ensure_column(connection, "eval_runs", "worker_id", "TEXT")
            ensure_column(connection, "eval_runs", "claimed_at", "TEXT")
            ensure_column(connection, "eval_runs", "heartbeat_at", "TEXT")
            ensure_column(connection, "eval_runs", "lease_expires_at", "TEXT")

    def claim_pending_run(self, worker_id: str | None = None) -> EvaluationRun | None:
        now = datetime.now(UTC)
        lease_expires = now + timedelta(seconds=300)
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
                SET status = ?, updated_at = ?, worker_id = ?, claimed_at = ?, lease_expires_at = ?
                WHERE id = ?
                """,
                (RunStatus.RUNNING.value, utc_now_iso(), worker_id, now.isoformat(), lease_expires.isoformat(), row["id"]),
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

    def heartbeat_run(self, run_id: str, worker_id: str, lease_expires_at: datetime) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE eval_runs
                SET worker_id = ?, heartbeat_at = ?, lease_expires_at = ?, updated_at = ?
                WHERE id = ?
                """,
                (worker_id, datetime.now(UTC).isoformat(), lease_expires_at.isoformat(), utc_now_iso(), run_id),
            )

    def complete_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.COMPLETED)

    def fail_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.FAILED)

    def retry_run(self, run: EvaluationRun) -> EvaluationRun:
        return self._finalize_run(run, RunStatus.PENDING)

    def _finalize_run(self, run: EvaluationRun, target_status: RunStatus) -> EvaluationRun:
        with self._connect() as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = connection.execute(
                "SELECT status FROM eval_runs WHERE id = ?", (run.run_id,)
            ).fetchone()
            if row is None:
                connection.execute("ROLLBACK")
                raise KeyError(f"Unknown run id: {run.run_id}")
            current_status = RunStatus(row["status"])
            if current_status != RunStatus.RUNNING:
                connection.execute("ROLLBACK")
                return self.get_run(run.run_id)
            run = run.model_copy(update={"status": target_status})
            self._write_run_artifacts(run, connection)
            self._upsert_run_connection(connection, run)
            connection.execute("DELETE FROM eval_run_tasks WHERE run_id = ?", (run.run_id,))
            for trace in run.traces:
                self._upsert_task(connection, run.run_id, trace)
            connection.execute("COMMIT")
        return self.get_run(run.run_id)

    def _upsert_run(self, run: EvaluationRun) -> None:
        with self._connect() as connection:
            self._upsert_run_connection(connection, run)

    def _upsert_run_connection(self, connection: sqlite3.Connection, run: EvaluationRun) -> None:
        report = build_report(run.traces)
        provider = run.provider or first_trace_attr(run.traces, "provider")
        model = run.model or first_trace_attr(run.traces, "model")
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
                failure_categories_json, metrics_json, started_at, ended_at, duration_ms,
                retry_count, max_retries, failure_reason, failure_category,
                worker_id, claimed_at, heartbeat_at, lease_expires_at,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                duration_ms = excluded.duration_ms,
                retry_count = excluded.retry_count,
                max_retries = excluded.max_retries,
                failure_reason = excluded.failure_reason,
                failure_category = excluded.failure_category,
                worker_id = excluded.worker_id,
                claimed_at = excluded.claimed_at,
                heartbeat_at = excluded.heartbeat_at,
                lease_expires_at = excluded.lease_expires_at,
                updated_at = excluded.updated_at
            """,
            (
                run.run_id,
                run.status.value,
                json.dumps(run.requested_task_ids),
                provider,
                model,
                report.success_rate,
                run.case_source,
                report.verification_pass_rate,
                report.scope_violation_rate,
                json.dumps(report.failure_categories),
                run.metrics.model_dump_json(),
                run.started_at.isoformat(),
                run.ended_at.isoformat(),
                run.duration_ms,
                run.retry_count,
                run.max_retries,
                run.failure_reason,
                run.failure_category.value,
                run.worker_id,
                run.claimed_at.isoformat() if run.claimed_at is not None else None,
                run.heartbeat_at.isoformat() if run.heartbeat_at is not None else None,
                run.lease_expires_at.isoformat() if run.lease_expires_at is not None else None,
                created_at,
                now,
            ),
        )

    def _write_run_artifacts(self, run: EvaluationRun, connection: sqlite3.Connection | None = None) -> None:
        run_artifacts_path = self.artifacts_path / run.run_id
        run_artifacts_path.mkdir(parents=True, exist_ok=True)
        trace_path = run_artifacts_path / "trace.json"
        report_path = run_artifacts_path / "report.json"
        trace_path.write_text(
            json.dumps([trace.model_dump(mode="json") for trace in run.traces], indent=2),
            encoding="utf-8",
        )
        report_path.write_text(
            build_report(run.traces).model_dump_json(indent=2),
            encoding="utf-8",
        )
        if connection is not None:
            self._upsert_artifact(
                connection,
                artifact_for_path(run.run_id, "trace", trace_path),
            )
            self._upsert_artifact(
                connection,
                artifact_for_path(run.run_id, "report", report_path),
            )
        else:
            with self._connect() as connection:
                self._upsert_artifact(
                    connection,
                    artifact_for_path(run.run_id, "trace", trace_path),
                )
                self._upsert_artifact(
                    connection,
                    artifact_for_path(run.run_id, "report", report_path),
                )

    def _read_trace_artifact(self, run_id: str) -> list[AgentTrace]:
        trace_artifacts = [
            artifact for artifact in self.list_artifacts(run_id) if artifact.kind == "trace"
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
            INSERT INTO eval_artifacts (id, run_id, kind, path, size_bytes, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                run_id = excluded.run_id,
                kind = excluded.kind,
                path = excluded.path,
                size_bytes = excluded.size_bytes,
                created_at = excluded.created_at
            """,
            (
                artifact.id,
                artifact.run_id,
                artifact.kind,
                artifact.path,
                artifact.size_bytes,
                artifact.created_at.isoformat(),
            ),
        )

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.db_path)
        connection.row_factory = sqlite3.Row
        return connection


def replace_trace(traces: Sequence[AgentTrace], trace: AgentTrace) -> list[AgentTrace]:
    filtered = [existing for existing in traces if existing.task_id != trace.task_id]
    return [*filtered, trace]


def optional_bool_to_int(value: bool | None) -> int | None:
    if value is None:
        return None
    return int(value)


def first_trace_attr(traces: Sequence[AgentTrace], attr: str) -> str | None:
    if not traces:
        return None
    value = getattr(traces[0], attr)
    return str(value) if value is not None else None


def artifact_for_path(run_id: str, kind: str, path: Path) -> EvalArtifact:
    return EvalArtifact(
        id=f"{run_id}:{kind}",
        run_id=run_id,
        kind=kind,
        path=str(path),
        size_bytes=path.stat().st_size,
        created_at=datetime.now(UTC),
    )


def artifact_from_row(row: sqlite3.Row) -> EvalArtifact:
    return EvalArtifact(
        id=row["id"],
        run_id=row["run_id"],
        kind=row["kind"],
        path=row["path"],
        size_bytes=row["size_bytes"],
        created_at=datetime.fromisoformat(row["created_at"]),
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
