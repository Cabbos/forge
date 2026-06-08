import argparse
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
    ) -> None:
        self.storage = storage
        self.forge_command = forge_command
        self.worker_id = worker_id
        self.heartbeat_interval_seconds = heartbeat_interval_seconds

    def run_once(self) -> EvaluationRun | None:
        run = self.storage.claim_pending_run(worker_id=self.worker_id)
        if run is None:
            return None

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
                    return cancelled_run

                trace = runner.run_task(task)

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
                    return cancelled_run

                traces.append(trace)
                self.storage.save_task(run.run_id, trace)

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
                return result
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
                    return result
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
                return result
            return result
        finally:
            heartbeat_stop.set()

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
        default=5.0,
        help="Delay between polling attempts when not running with --once.",
    )
    args = parser.parse_args(argv)

    settings = get_settings()
    worker = EvalWorker(
        storage=build_storage(settings),
        forge_command=settings.forge_agent_command,
    )

    if args.once:
        worker.run_once()
        return 0

    while True:
        worker.run_once()
        time.sleep(args.poll_interval_seconds)


if __name__ == "__main__":
    raise SystemExit(main())
