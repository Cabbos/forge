import subprocess
import sys
from pathlib import Path

from app.cases import load_cases, validate_case_quality
from app.datasets import dataset_fingerprint
from app.forgebench import freeze_dataset, verify_catalog
from app.hidden_verifiers import run_hidden_verifier
from app.models import DatasetSplit
from app.runner import prepare_workspace, run_shell_commands

FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
DEVELOPMENT_CASES = FORGEBENCH_ROOT / "cases" / "development"
HOLDOUT_CASES = FORGEBENCH_ROOT / "cases" / "holdout"
FIXTURE_LOCK = FORGEBENCH_ROOT / "fixture-lock.json"
VERIFIERS = FORGEBENCH_ROOT / "verifiers"

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

EXPECTED_HOLDOUT_CASES = {
    "forgebench-holdout-domain-due-date-zone",
    "forgebench-holdout-domain-stable-sort",
    "forgebench-holdout-web-saved-filters",
    "forgebench-holdout-web-optimistic-rollback",
    "forgebench-holdout-admin-pagination-query",
    "forgebench-holdout-admin-permission-guard",
    "forgebench-holdout-api-validation",
    "forgebench-holdout-api-conditional-update",
    "forgebench-holdout-worker-quarantine",
    "forgebench-holdout-worker-dedupe",
    "forgebench-holdout-ui-empty-state-escape",
    "forgebench-holdout-ui-keyboard-shortcut",
    "forgebench-holdout-config-port-range",
    "forgebench-holdout-config-feature-flag",
    "forgebench-holdout-release-version-match",
    "forgebench-holdout-release-checksums",
    "forgebench-holdout-contract-task-forward-compat",
    "forgebench-holdout-integration-bulk-edit",
    "forgebench-holdout-ambiguous-notification-badge",
    "forgebench-holdout-long-context-audit-log",
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


def test_holdout_catalog_has_exact_sealed_inventory() -> None:
    development_ids = {task.id for task in load_cases(DEVELOPMENT_CASES)}
    tasks = load_cases(HOLDOUT_CASES)

    assert {task.id for task in tasks} == EXPECTED_HOLDOUT_CASES
    assert len(tasks) == 20
    assert development_ids.isdisjoint(EXPECTED_HOLDOUT_CASES)
    assert validate_case_quality(tasks) == []
    assert sum("long-context" in task.tags for task in tasks) == 1
    assert sum("python" in task.tags for task in tasks) >= 6
    assert sum("typescript" in task.tags for task in tasks) >= 8
    assert sum("multi-file" in task.tags for task in tasks) >= 5
    assert sum(task.metadata.get("tempting_forbidden_path") is True for task in tasks) >= 4
    for task in tasks:
        assert task.fixture_id == "forgebench-monorepo"
        assert task.fixture_version == "1.0.0"
        assert task.fixture_path == str(FIXTURE.resolve())
        assert task.scenario_path is not None and Path(task.scenario_path).is_dir()
        assert task.dataset_split == DatasetSplit.HOLDOUT
        assert task.hidden_verifier_id
        assert task.full_verification_command == "python3 tools/release/verify.py"
        assert task.metadata["real_coding"] is True
        assert task.metadata["holdout_unsealed"] is False
        assert task.metadata["development_visible"] is False
        assert "mock" not in task.metadata
        assert task.max_model_rounds <= 48
        if "long-context" in task.tags:
            assert task.max_model_rounds == 48
        else:
            assert task.max_model_rounds <= 40


def test_holdout_scenarios_fail_opaque_verifiers_but_keep_unrelated_checks(
    tmp_path: Path,
) -> None:
    for task in load_cases(HOLDOUT_CASES):
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

        assert hidden.exit_code != 0, task.id
        assert unrelated
        assert all(output.exit_code == 0 for output in unrelated), task.id


def test_holdout_prompts_and_context_do_not_leak_implementation_targets() -> None:
    for task in load_cases(HOLDOUT_CASES):
        prompt = task.prompt.lower()

        assert task.context_files == []
        assert all(suffix not in prompt for suffix in (".ts", ".tsx", ".py", ".json", ".toml"))
        assert "/" not in prompt
        assert task.hidden_verifier_id is not None
        assert task.hidden_verifier_id.startswith("h")
        assert task.hidden_verifier_id[1:].isdigit()


def test_dataset_lock_freezes_exact_split_and_verifier_contents(tmp_path: Path) -> None:
    output = tmp_path / "dataset-lock.json"
    payload = freeze_dataset(FORGEBENCH_ROOT / "cases", FIXTURE_LOCK, output)
    development = load_cases(DEVELOPMENT_CASES)
    holdout = load_cases(HOLDOUT_CASES)

    assert output.exists()
    assert set(payload) == {
        "schema_version",
        "fixture",
        "development",
        "holdout",
        "combined_fingerprint",
        "verifier_tree_sha256",
    }
    assert payload["schema_version"] == 1
    assert payload["fixture"]["fixture_id"] == "forgebench-monorepo"
    assert payload["fixture"]["fixture_version"] == "1.0.0"
    assert payload["development"] == {
        "count": 10,
        "ids": sorted(task.id for task in development),
        "fingerprint": dataset_fingerprint(development),
    }
    assert payload["holdout"] == {
        "count": 20,
        "ids": sorted(task.id for task in holdout),
        "fingerprint": dataset_fingerprint(holdout),
    }
    assert payload["combined_fingerprint"] == dataset_fingerprint([*development, *holdout])
    assert len(payload["verifier_tree_sha256"]) == 64


def test_verify_catalog_prints_only_counts_and_hashes(
    tmp_path: Path,
    capsys,
) -> None:
    output = tmp_path / "dataset-lock.json"
    payload = freeze_dataset(FORGEBENCH_ROOT / "cases", FIXTURE_LOCK, output)

    summary = verify_catalog(FORGEBENCH_ROOT / "cases", output)

    assert summary["development_count"] == 10
    assert summary["holdout_count"] == 20
    assert summary["combined_fingerprint"] == payload["combined_fingerprint"]
    rendered = capsys.readouterr().out
    assert rendered == ""
    for task in load_cases(HOLDOUT_CASES):
        assert task.prompt not in str(summary)


def test_verify_catalog_rejects_content_drift(tmp_path: Path) -> None:
    output = tmp_path / "dataset-lock.json"
    freeze_dataset(FORGEBENCH_ROOT / "cases", FIXTURE_LOCK, output)
    lock = output.read_text(encoding="utf-8")
    output.write_text(lock.replace('"count": 20', '"count": 19'), encoding="utf-8")

    try:
        verify_catalog(FORGEBENCH_ROOT / "cases", output)
    except ValueError as exc:
        assert "dataset lock does not match catalog contents" in str(exc)
    else:
        raise AssertionError("drifted dataset lock was accepted")


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
