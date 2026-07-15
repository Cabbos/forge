import argparse
import signal
import sqlite3
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

from app.config import get_settings
from app.execution import ExecutionOptions, execute_evaluation
from app.main import build_storage
from app.metrics import calculate_metrics
from app.models import (
    AgentTrace,
    EvaluationRun,
    FailureCategory,
    RunStatus,
    TrustGateResult,
)
from app.runner import validate_execution_identity
from app.storage import EvalStorage, LeaseLostError
from app.trace import duration_ms, utc_now


class EvalWorker:
    """Small local worker that claims one pending run and executes it synchronously."""

    def __init__(
        self,
        *,
        storage: EvalStorage,
        forge_command: str | None,
        worker_id: str = "local-worker",
        heartbeat_interval_seconds: float = 30.0,
        poll_interval_seconds: float = 5.0,
        command_timeout_seconds: float = 900.0,
        setup_timeout_seconds: float = 300.0,
        validation_timeout_seconds: float = 300.0,
        lease_duration_seconds: float = 300.0,
    ) -> None:
        self.storage = storage
        self.forge_command = forge_command
        self.worker_id = worker_id
        self.heartbeat_interval_seconds = heartbeat_interval_seconds
        self.poll_interval_seconds = poll_interval_seconds
        self.command_timeout_seconds = command_timeout_seconds
        self.setup_timeout_seconds = setup_timeout_seconds
        self.validation_timeout_seconds = validation_timeout_seconds
        self.lease_duration_seconds = lease_duration_seconds
        self._stop_event = threading.Event()

    @property
    def should_stop(self) -> bool:
        return self._stop_event.is_set()

    def stop(self) -> None:
        """Signal the worker to stop and cancel in-flight execution."""
        self._stop_event.set()

    def run_once(self) -> EvaluationRun | None:
        run = self.storage.claim_pending_run(
            worker_id=self.worker_id,
            lease_duration_seconds=self.lease_duration_seconds,
        )
        if run is None:
            return None
        if run.worker_id != self.worker_id or run.lease_token is None:
            raise LeaseLostError(f"Worker {self.worker_id} lost lease for run {run.run_id}")
        lease_token = run.lease_token
        self._log(f"claimed run {run.run_id} status={run.status.value}")

        started_at = utc_now()
        traces: list[AgentTrace] = []
        trust_result = TrustGateResult()
        heartbeat_stop, lease_lost = self._start_background_heartbeat(run.run_id, lease_token)

        def cancel_requested() -> bool:
            current = self.storage.get_run(run.run_id)
            return (
                self.should_stop
                or lease_lost.is_set()
                or current is None
                or current.status == RunStatus.CANCELLED
                or current.worker_id != self.worker_id
                or current.lease_token != lease_token
            )

        try:
            try:
                provider, model, case_source = validate_execution_identity(
                    run.provider,
                    run.model,
                    run.case_source,
                )
                tasks = self.storage.get_tasks(run.requested_task_ids)
                execution = execute_evaluation(
                    cases_path=Path(case_source),
                    tasks=tasks,
                    options=ExecutionOptions(
                        provider=provider,
                        model=model,
                        forge_command=self.forge_command,
                        command_timeout_seconds=self.command_timeout_seconds,
                        setup_timeout_seconds=self.setup_timeout_seconds,
                        validation_timeout_seconds=self.validation_timeout_seconds,
                        require_red_team=False,
                    ),
                    cancel_requested=cancel_requested,
                )
                traces = execution.traces
                trust_result = execution.trust_result
                for trace in traces:
                    self.storage.save_task(
                        run.run_id,
                        trace,
                        worker_id=self.worker_id,
                        lease_token=lease_token,
                    )

                current = self.storage.get_run(run.run_id)
                if current is None:
                    raise LeaseLostError(f"Worker {self.worker_id} lost lease for run {run.run_id}")
                if current.status == RunStatus.CANCELLED or self.should_stop:
                    if current.status != RunStatus.CANCELLED:
                        self.storage.cancel_run(run.run_id)
                    cancelled_run = self._build_run_result(
                        run,
                        traces,
                        started_at,
                        RunStatus.CANCELLED,
                        trust_result,
                    )
                    result = self.storage.complete_run(
                        cancelled_run,
                        worker_id=self.worker_id,
                        lease_token=lease_token,
                    )
                    self._log(f"cancelled run {run.run_id} tasks={len(traces)}")
                    return result
                if (
                    current.worker_id != self.worker_id
                    or current.lease_token != lease_token
                    or lease_lost.is_set()
                ):
                    raise LeaseLostError(f"Worker {self.worker_id} lost lease for run {run.run_id}")
                completed_run = self._build_run_result(
                    run,
                    traces,
                    started_at,
                    RunStatus.COMPLETED,
                    trust_result,
                )
                result = self.storage.complete_run(
                    completed_run,
                    worker_id=self.worker_id,
                    lease_token=lease_token,
                )
                if result.status == RunStatus.CANCELLED:
                    self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                    return result
                self._log(f"completed run {run.run_id} tasks={len(traces)}")
                return result
            except LeaseLostError:
                raise
            except Exception as exc:
                failure_reason = f"{type(exc).__name__}: {exc}"
                if run.retry_count < run.max_retries:
                    retry_run = self._build_run_result(
                        run.model_copy(update={"retry_count": run.retry_count + 1}),
                        traces,
                        started_at,
                        RunStatus.PENDING,
                        trust_result,
                        failure_reason,
                        FailureCategory.RUNNER_ERROR,
                    )
                    result = self.storage.retry_run(
                        retry_run,
                        worker_id=self.worker_id,
                        lease_token=lease_token,
                    )
                    if result.status == RunStatus.CANCELLED:
                        self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                        return result
                    self._log(
                        f"retried run {run.run_id} retry={result.retry_count}/{result.max_retries}"
                    )
                    return result
                failed_run = self._build_run_result(
                    run,
                    traces,
                    started_at,
                    RunStatus.FAILED,
                    trust_result,
                    failure_reason,
                    FailureCategory.RUNNER_ERROR,
                )
                result = self.storage.fail_run(
                    failed_run,
                    worker_id=self.worker_id,
                    lease_token=lease_token,
                )
                if result.status == RunStatus.CANCELLED:
                    self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                    return result
                self._log(
                    f"failed run {run.run_id} retries={result.retry_count}/{result.max_retries}"
                )
                return result
        except LeaseLostError:
            heartbeat_stop.set()
            self._log(f"lost lease for run {run.run_id}")
            return self.storage.get_run(run.run_id)
        finally:
            heartbeat_stop.set()

    def _build_run_result(
        self,
        run: EvaluationRun,
        traces: list[AgentTrace],
        started_at: datetime,
        status: RunStatus,
        trust_result: TrustGateResult,
        failure_reason: str | None = None,
        failure_category: FailureCategory = FailureCategory.NONE,
    ) -> EvaluationRun:
        ended_at = utc_now()
        return run.model_copy(
            update={
                "status": status,
                "traces": traces,
                "metrics": calculate_metrics(traces),
                "trust_result": trust_result,
                "started_at": started_at,
                "ended_at": ended_at,
                "duration_ms": duration_ms(started_at, ended_at),
                "failure_reason": failure_reason,
                "failure_category": failure_category,
            }
        )

    def _log(self, message: str) -> None:
        print(f"[worker {self.worker_id}] {message}", file=sys.stderr)

    def _start_background_heartbeat(
        self,
        run_id: str,
        lease_token: str,
    ) -> tuple[threading.Event, threading.Event]:
        """Start a background thread that heartbeats until the event is set."""
        stop_event = threading.Event()
        lease_lost_event = threading.Event()

        def heartbeat_loop() -> None:
            while not stop_event.wait(timeout=self.heartbeat_interval_seconds):
                try:
                    self._heartbeat(run_id, lease_token)
                except LeaseLostError:
                    lease_lost_event.set()
                    return
                except sqlite3.Error as exc:
                    self._log(f"heartbeat failed for run {run_id}: {type(exc).__name__}: {exc}")

        thread = threading.Thread(target=heartbeat_loop, daemon=True)
        thread.start()
        return stop_event, lease_lost_event

    def _heartbeat(self, run_id: str, lease_token: str) -> None:
        from datetime import UTC, datetime, timedelta

        expires = datetime.now(UTC) + timedelta(seconds=self.lease_duration_seconds)
        self.storage.heartbeat_run(run_id, self.worker_id, lease_token, expires)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run pending Forge eval jobs.")
    parser.add_argument("--once", action="store_true", help="Claim and execute at most one run.")
    parser.add_argument(
        "--poll-interval-seconds",
        type=float,
        default=None,
        help="Delay between polling attempts when not running with --once. Overrides env config.",
    )
    args = parser.parse_args(argv)

    settings = get_settings()
    worker = EvalWorker(
        storage=build_storage(settings),
        forge_command=settings.forge_agent_command,
        worker_id=settings.worker_id,
        heartbeat_interval_seconds=settings.heartbeat_interval_seconds,
        poll_interval_seconds=args.poll_interval_seconds or settings.poll_interval_seconds,
        command_timeout_seconds=settings.command_timeout_seconds,
        setup_timeout_seconds=settings.setup_timeout_seconds,
        validation_timeout_seconds=settings.validation_timeout_seconds,
        lease_duration_seconds=settings.lease_duration_seconds,
    )

    # Register graceful shutdown handlers
    def _signal_handler(_signum: int, _frame: object) -> None:
        print(
            f"[worker {worker.worker_id}] Received shutdown signal, stopping gracefully...",
            file=sys.stderr,
        )
        worker.stop()

    signal.signal(signal.SIGTERM, _signal_handler)
    signal.signal(signal.SIGINT, _signal_handler)

    if args.once:
        worker.run_once()
        return 0

    while not worker.should_stop:
        worker.run_once()
        if worker.should_stop:
            break
        time.sleep(worker.poll_interval_seconds)

    print(f"[worker {worker.worker_id}] Stopped.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
