from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Runtime settings for the eval runner service."""

    app_name: str = "forge-eval-runner"
    tasks_path: Path = Path("tasks/sample_tasks.json")
    storage_backend: str = "memory"
    db_path: Path = Path("forge_eval.db")
    artifacts_path: Path = Path("artifacts")
    run_execution_mode: str = "sync"
    forge_agent_command: str | None = None
    worker_id: str = "local-worker"
    heartbeat_interval_seconds: float = 30.0
    poll_interval_seconds: float = 5.0
    command_timeout_seconds: float = 900.0
    setup_timeout_seconds: float = 300.0
    validation_timeout_seconds: float = 300.0
    lease_duration_seconds: float = 300.0

    model_config = SettingsConfigDict(env_prefix="FORGE_EVAL_", env_file=".env", extra="ignore")


def get_settings() -> Settings:
    return Settings()
