import subprocess
import sys
from pathlib import Path

from app.cases import load_cases, validate_case_quality
from app.hidden_verifiers import run_hidden_verifier
from app.models import DatasetSplit
from app.runner import prepare_workspace, run_shell_commands

FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
DEVELOPMENT_CASES = FORGEBENCH_ROOT / "cases" / "development"

EXPECTED_DEVELOPMENT_CASES = {
    "forgebench-dev-domain-normalize-title": ["packages/domain/src/task-title.ts"],
    "forgebench-dev-web-filter-query": [
        "apps/web-console/src/task-list.tsx",
        "packages/domain/src/task-query.ts",
    ],
    "forgebench-dev-admin-bulk-status": [
        "apps/admin-console/src/bulk-actions.ts",
        "packages/domain/src/task.ts",
    ],
    "forgebench-dev-task-api-bulk-update": [
        "services/task-api/task_api/service.py",
        "services/task-api/task_api/repository.py",
    ],
    "forgebench-dev-worker-retry-backoff": ["services/event-worker/event_worker/retry.py"],
    "forgebench-dev-ui-priority-badge": ["packages/ui/src/priority-badge.tsx"],
    "forgebench-dev-config-env-parser": ["packages/config/src/env.ts"],
    "forgebench-dev-release-artifact-manifest": ["tools/release/build-manifest.ts"],
    "forgebench-dev-contract-event-schema": [
        "packages/domain/src/task.ts",
        "services/event-worker/event_worker/consumer.py",
    ],
    "forgebench-dev-idempotent-no-op": [],
}


def test_development_catalog_has_exact_canonical_inventory() -> None:
    tasks = load_cases(DEVELOPMENT_CASES)
    by_id = {task.id: task for task in tasks}

    assert set(by_id) == set(EXPECTED_DEVELOPMENT_CASES)
    assert len(tasks) == 10
    assert validate_case_quality(tasks) == []
    for task_id, allowed_paths in EXPECTED_DEVELOPMENT_CASES.items():
        task = by_id[task_id]
        assert task.fixture_id == "forgebench-monorepo"
        assert task.fixture_version == "1.0.0"
        assert task.fixture_path == str(FIXTURE.resolve())
        assert task.dataset_split == DatasetSplit.DEVELOPMENT
        assert task.hidden_verifier_id
        assert task.full_verification_command == "python3 tools/release/verify.py"
        assert task.validation_commands and len(task.validation_commands) == 1
        assert task.expected_files_changed == allowed_paths
        assert task.metadata["allowed_source_paths"] == allowed_paths
        assert task.metadata["real_coding"] is True
        assert task.metadata["holdout_unsealed"] is False
        assert task.forbidden_files_changed == [
            "package.json",
            "pyproject.toml",
            "tools/release/verify.py",
            ".forge/**",
        ]
        assert task.required_scores == ["functional_correctness", "scope_ok"]
        assert task.max_duration_seconds == 900
        assert task.max_model_rounds == 40
        if task_id.endswith("idempotent-no-op"):
            assert task.scenario_path is None
        else:
            assert task.scenario_path is not None
            assert Path(task.scenario_path).is_dir()


def test_development_scenarios_have_expected_polarity(tmp_path: Path) -> None:
    tasks = load_cases(DEVELOPMENT_CASES)
    for task in tasks:
        workspace = prepare_workspace(task, tmp_path / task.id)
        hidden = run_hidden_verifier(
            task.hidden_verifier_id or "",
            workspace,
            timeout_seconds=30,
        )
        unrelated = run_shell_commands(
            task.pass_to_pass_commands,
            workspace,
            timeout_seconds=30,
        )

        if task.id.endswith("idempotent-no-op"):
            assert hidden.exit_code == 0, hidden.stderr
        else:
            assert hidden.exit_code != 0, task.id
        assert unrelated
        assert all(output.exit_code == 0 for output in unrelated), task.id


def test_development_visible_checks_match_hidden_polarity(tmp_path: Path) -> None:
    tasks = load_cases(DEVELOPMENT_CASES)
    for task in tasks:
        workspace = prepare_workspace(task, tmp_path / task.id)
        visible = run_shell_commands(
            task.validation_commands,
            workspace,
            timeout_seconds=30,
        )

        assert len(visible) == 1
        if task.id.endswith("idempotent-no-op"):
            assert visible[0].exit_code == 0, visible[0].stderr
        else:
            assert visible[0].exit_code != 0, task.id


def test_development_scenarios_do_not_replace_root_or_embed_answers() -> None:
    forbidden_roots = {
        "package.json",
        "pyproject.toml",
        "tools/release/verify.py",
    }
    for scenario in DEVELOPMENT_CASES.glob("*/scenario"):
        relative_files = {
            path.relative_to(scenario).as_posix() for path in scenario.rglob("*") if path.is_file()
        }
        assert relative_files.isdisjoint(forbidden_roots)
        for path in scenario.rglob("*"):
            if path.is_file():
                content = path.read_text(encoding="utf-8").lower()
                assert "solution:" not in content
                assert "benchmark answer" not in content


def test_canonical_fixture_baseline_still_passes_full_verification() -> None:
    completed = subprocess.run(
        [sys.executable, "tools/release/verify.py"],
        cwd=FIXTURE,
        text=True,
        capture_output=True,
        timeout=120,
        check=False,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
