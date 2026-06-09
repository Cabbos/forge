from uuid import uuid4

from fastapi import FastAPI, HTTPException, status

from app.config import get_settings
from app.metrics import calculate_metrics
from app.models import (
    AgentTrace,
    BacktestReport,
    EvalArtifact,
    EvaluationRun,
    EvaluationTask,
    MetricsSummary,
    RunCreateRequest,
    RunStatus,
)
from app.reporting import build_report
from app.runner import create_runner
from app.storage import EvalStorage, InMemoryStorage, RunAlreadyTerminalError, SQLiteStorage
from app.trace import duration_ms, utc_now


def build_storage(settings) -> EvalStorage:
    if settings.storage_backend == "sqlite":
        return SQLiteStorage(
            tasks_path=settings.tasks_path,
            db_path=settings.db_path,
            artifacts_path=settings.artifacts_path,
        )
    if settings.storage_backend == "memory":
        return InMemoryStorage(tasks_path=settings.tasks_path)
    raise ValueError(f"Unsupported storage backend: {settings.storage_backend}")


def create_app(storage: EvalStorage | None = None) -> FastAPI:
    settings = get_settings()
    app = FastAPI(
        title="Forge Eval Runner",
        summary="Agent evaluation runner with trace capture and metrics APIs.",
        version="0.1.0",
    )
    app.state.storage = storage or build_storage(settings)

    def get_storage() -> EvalStorage:
        return app.state.storage

    def require_run(run_id: str) -> EvaluationRun:
        run = get_storage().get_run(run_id)
        if run is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Run not found")
        return run

    @app.get("/health")
    def health() -> dict[str, str]:
        return {"status": "ok", "service": settings.app_name}

    @app.get("/tasks", response_model=list[EvaluationTask])
    def list_tasks() -> list[EvaluationTask]:
        return get_storage().list_tasks()

    @app.post("/runs", response_model=EvaluationRun, status_code=status.HTTP_201_CREATED)
    def create_run(request: RunCreateRequest) -> EvaluationRun:
        try:
            tasks = get_storage().get_tasks(request.task_ids)
        except KeyError as exc:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail=str(exc)) from exc

        started_at = utc_now()
        if settings.run_execution_mode == "queued":
            run = EvaluationRun(
                run_id=str(uuid4()),
                status=RunStatus.PENDING,
                provider=request.provider,
                model=request.model,
                case_source=str(settings.tasks_path),
                requested_task_ids=[task.id for task in tasks],
                traces=[],
                metrics=calculate_metrics([]),
                started_at=started_at,
                ended_at=started_at,
                duration_ms=0,
                max_retries=request.max_retries,
            )
            get_storage().create_run(run)
            return run
        if settings.run_execution_mode != "sync":
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail=f"Unsupported run execution mode: {settings.run_execution_mode}",
            )

        runner = create_runner(
            provider=request.provider,
            model=request.model,
            forge_command=settings.forge_agent_command,
        )
        traces = [runner.run_task(task) for task in tasks]
        ended_at = utc_now()
        run = EvaluationRun(
            run_id=str(uuid4()),
            status=RunStatus.COMPLETED,
            provider=request.provider,
            model=request.model,
            case_source=str(settings.tasks_path),
            requested_task_ids=[task.id for task in tasks],
            traces=traces,
            metrics=calculate_metrics(traces),
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )
        get_storage().save_run(run)
        return run

    @app.get("/runs", response_model=list[EvaluationRun])
    def list_runs(status: str | None = None) -> list[EvaluationRun]:
        return get_storage().list_runs(status_filter=status)

    @app.get("/runs/{run_id}", response_model=EvaluationRun)
    def get_run(run_id: str) -> EvaluationRun:
        return require_run(run_id)

    @app.get("/runs/{run_id}/trace", response_model=list[AgentTrace])
    def get_run_trace(run_id: str) -> list[AgentTrace]:
        return require_run(run_id).traces

    @app.get("/runs/{run_id}/metrics", response_model=MetricsSummary)
    def get_run_metrics(run_id: str) -> MetricsSummary:
        return require_run(run_id).metrics

    @app.get("/runs/{run_id}/report", response_model=BacktestReport)
    def get_run_report(run_id: str) -> BacktestReport:
        return build_report(require_run(run_id).traces)

    @app.get("/runs/{run_id}/artifacts", response_model=list[EvalArtifact])
    def get_run_artifacts(run_id: str) -> list[EvalArtifact]:
        require_run(run_id)
        return get_storage().list_artifacts(run_id)

    @app.post("/runs/{run_id}/cancel", response_model=EvaluationRun)
    def cancel_run(run_id: str) -> EvaluationRun:
        require_run(run_id)
        try:
            cancelled = get_storage().cancel_run(run_id)
        except RunAlreadyTerminalError as exc:
            raise HTTPException(
                status_code=status.HTTP_409_CONFLICT,
                detail=str(exc),
            ) from exc
        if cancelled is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Run not found")
        return cancelled

    return app


app = create_app()
