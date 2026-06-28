import argparse
import signal
import sys
import threading
import time

from app.config import get_settings
from app.main import build_storage
from app.metrics import calculate_metrics
from app.models import EvaluationRun, FailureCategory, RunStatus
from app.runner import create_runner
from app.storage import EvalStorage
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
    ) -> None:
        self.storage = storage
        self.forge_command = forge_command
        self.worker_id = worker_id
        self.heartbeat_interval_seconds = heartbeat_interval_seconds
        self.poll_interval_seconds = poll_interval_seconds
        self._stop_event = threading.Event()

    @property
    def should_stop(self) -> bool:
        return self._stop_event.is_set()

    def stop(self) -> None:
        """Signal the worker to stop after the current poll iteration."""
        self._stop_event.set()

    def run_once(self) -> EvaluationRun | None:
        run = self.storage.claim_pending_run(worker_id=self.worker_id)
        if run is None:
            return None
        self._log(f"claimed run {run.run_id} status={run.status.value}")

        started_at = utc_now()
        traces = []
        heartbeat_stop = self._start_background_heartbeat(run.run_id)
        try:
            runner = create_runner(
                provider=run.provider,
                model=run.model,
                forge_command=self.forge_command,
            )
            for task in self.storage.get_tasks(run.requested_task_ids):
                # Check cancellation at task boundary (before)
                current = self.storage.get_run(run.run_id)
                if current is not None and current.status == RunStatus.CANCELLED:
                    ended_at = utc_now()
                    cancelled_run = run.model_copy(
                        update={
                            "status": RunStatus.CANCELLED,
                            "traces": traces,
                            "metrics": calculate_metrics(traces),
                            "started_at": started_at,
                            "ended_at": ended_at,
                            "duration_ms": duration_ms(started_at, ended_at),
                        }
                    )
                    self.storage.save_run(cancelled_run)
                    self._log(f"cancelled run {run.run_id} tasks={len(traces)}")
                    return cancelled_run

                trace = runner.run_task(task)
                traces.append(trace)
                self.storage.save_task(run.run_id, trace)

                # Check cancellation at task boundary (after task returns)
                current = self.storage.get_run(run.run_id)
                if current is not None and current.status == RunStatus.CANCELLED:
                    ended_at = utc_now()
                    cancelled_run = run.model_copy(
                        update={
                            "status": RunStatus.CANCELLED,
                            "traces": traces,
                            "metrics": calculate_metrics(traces),
                            "started_at": started_at,
                            "ended_at": ended_at,
                            "duration_ms": duration_ms(started_at, ended_at),
                        }
                    )
                    self.storage.save_run(cancelled_run)
                    self._log(f"cancelled run {run.run_id} tasks={len(traces)}")
                    return cancelled_run

            ended_at = utc_now()
            completed_run = run.model_copy(
                update={
                    "status": RunStatus.COMPLETED,
                    "traces": traces,
                    "metrics": calculate_metrics(traces),
                    "started_at": started_at,
                    "ended_at": ended_at,
                    "duration_ms": duration_ms(started_at, ended_at),
                }
            )
            result = self.storage.complete_run(completed_run)
            if result.status == RunStatus.CANCELLED:
                self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                return result
            self._log(f"completed run {run.run_id} tasks={len(traces)}")
            return result
        except Exception as exc:
            ended_at = utc_now()
            failure_reason = f"{type(exc).__name__}: {exc}"
            if run.retry_count < run.max_retries:
                retry_run = run.model_copy(
                    update={
                        "status": RunStatus.PENDING,
                        "traces": traces,
                        "metrics": calculate_metrics(traces),
                        "started_at": started_at,
                        "ended_at": ended_at,
                        "duration_ms": duration_ms(started_at, ended_at),
                        "retry_count": run.retry_count + 1,
                        "failure_reason": failure_reason,
                        "failure_category": FailureCategory.RUNNER_ERROR,
                    }
                )
                result = self.storage.retry_run(retry_run)
                if result.status == RunStatus.CANCELLED:
                    self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                    return result
                self._log(
                    f"retried run {run.run_id} retry={result.retry_count}/{result.max_retries}"
                )
                return result
            failed_run = run.model_copy(
                update={
                    "status": RunStatus.FAILED,
                    "traces": traces,
                    "metrics": calculate_metrics(traces),
                    "started_at": started_at,
                    "ended_at": ended_at,
                    "duration_ms": duration_ms(started_at, ended_at),
                    "failure_reason": failure_reason,
                    "failure_category": FailureCategory.RUNNER_ERROR,
                }
            )
            result = self.storage.fail_run(failed_run)
            if result.status == RunStatus.CANCELLED:
                self._log(f"cancelled run {run.run_id} tasks={len(result.traces)}")
                return result
            self._log(f"failed run {run.run_id} retries={result.retry_count}/{result.max_retries}")
            return result
        finally:
            heartbeat_stop.set()

    def _log(self, message: str) -> None:
        print(f"[worker {self.worker_id}] {message}", file=sys.stderr)

    def _start_background_heartbeat(self, run_id: str) -> threading.Event:
        """Start a background thread that heartbeats until the event is set."""
        stop_event = threading.Event()

        def heartbeat_loop() -> None:
            while not stop_event.wait(timeout=self.heartbeat_interval_seconds):
                try:
                    self._heartbeat(run_id)
                except Exception:
                    # Heartbeat failures are non-fatal; the lease may expire
                    # and another worker can reclaim the run.
                    pass

        thread = threading.Thread(target=heartbeat_loop, daemon=True)
        thread.start()
        return stop_event

    def _heartbeat(self, run_id: str) -> None:
        from datetime import UTC, datetime, timedelta
        expires = datetime.now(UTC) + timedelta(seconds=300)
        self.storage.heartbeat_run(run_id, self.worker_id, expires)


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
    )

    # Register graceful shutdown handlers
    def _signal_handler(_signum: int, _frame: object) -> None:
        print(f"[worker {worker.worker_id}] Received shutdown signal, stopping gracefully...", file=sys.stderr)
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
