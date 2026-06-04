import argparse
import time

from app.config import get_settings
from app.main import build_storage
from app.metrics import calculate_metrics
from app.models import EvaluationRun, RunStatus
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
    ) -> None:
        self.storage = storage
        self.forge_command = forge_command

    def run_once(self) -> EvaluationRun | None:
        run = self.storage.claim_pending_run()
        if run is None:
            return None

        started_at = utc_now()
        traces = []
        try:
            runner = create_runner(
                provider=run.provider,
                model=run.model,
                forge_command=self.forge_command,
            )
            for task in self.storage.get_tasks(run.requested_task_ids):
                trace = runner.run_task(task)
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
            self.storage.save_run(completed_run)
            return completed_run
        except Exception:
            ended_at = utc_now()
            failed_run = run.model_copy(
                update={
                    "status": RunStatus.FAILED,
                    "traces": traces,
                    "metrics": calculate_metrics(traces),
                    "started_at": started_at,
                    "ended_at": ended_at,
                    "duration_ms": duration_ms(started_at, ended_at),
                }
            )
            self.storage.save_run(failed_run)
            raise


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
