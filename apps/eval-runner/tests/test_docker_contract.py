from pathlib import Path


def test_compose_defines_authenticated_api_and_worker_with_shared_storage() -> None:
    compose = Path("docker-compose.yml").read_text(encoding="utf-8")
    assert "forge-eval-api:" in compose
    assert "forge-eval-worker:" in compose
    assert "FORGE_EVAL_RUN_EXECUTION_MODE: queued" in compose
    assert "FORGE_EVAL_API_BIND_HOST: 0.0.0.0" in compose
    assert "FORGE_EVAL_API_TOKEN" in compose
    assert "eval-db:/data" in compose
    assert "eval-artifacts:/artifacts" in compose
    assert "python -m app.worker" in compose


def test_dockerfile_contains_cases_for_harness_checks() -> None:
    dockerfile = Path("Dockerfile").read_text(encoding="utf-8")
    assert "COPY eval_cases ./eval_cases" in dockerfile
