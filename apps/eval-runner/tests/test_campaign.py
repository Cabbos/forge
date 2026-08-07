import fcntl
import json
import os
import shutil
import stat
import subprocess
from datetime import UTC, datetime
from pathlib import Path

import pytest

from app.artifact_hashing import canonical_tree_digest
from app.campaign import (
    CampaignCrashInjected,
    accept_campaign_patch,
    initialize_campaign,
    recover_pending_accept,
)
from app.cases import load_cases
from app.models import (
    AgentTrace,
    EvalProvider,
    FileDiff,
    LeakageCheck,
    TrustGateResult,
    TrustStatus,
    VerificationResult,
    WorkspaceCheck,
    WorkspaceObservation,
)
from app.provenance import sha256_bytes, verify_ledger
from app.reporting import build_report

FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
FIXTURE_LOCK = FORGEBENCH_ROOT / "fixture-lock.json"
CASES = FORGEBENCH_ROOT / "cases"
DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"


def write_harness_profile(path: Path, *, secret: bool = False) -> None:
    payload = {
        "schema_version": 1,
        "profile_id": "deepseek-v4-flash-frontier-v1",
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic-compatible",
        "tool_policy": "forge-default",
        "max_model_rounds": 40,
    }
    if secret:
        payload["api_key"] = "sk-" + "x" * 32
    path.write_text(json.dumps(payload), encoding="utf-8")


def initialize(
    tmp_path: Path,
    *,
    fixture: Path = FIXTURE,
    fixture_lock: Path = FIXTURE_LOCK,
    cases: Path = CASES,
    dataset_lock: Path = DATASET_LOCK,
    harness_profile: Path | None = None,
) -> Path:
    campaign = tmp_path / "campaign"
    profile = harness_profile or tmp_path / "harness-profile.json"
    if harness_profile is None:
        write_harness_profile(profile)
    initialize_campaign(
        campaign,
        campaign_id="campaign-test",
        fixture=fixture,
        fixture_lock=fixture_lock,
        cases=cases,
        dataset_lock=dataset_lock,
        harness_profile=profile,
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic-compatible",
        controller_version="forge-eval-controller/1",
        runtime_version="forge-runtime/1",
    )
    return campaign


def git(worktree: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=worktree,
        text=True,
        capture_output=True,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    return completed.stdout.strip()


def test_initialize_campaign_creates_isolated_locked_baseline(tmp_path: Path) -> None:
    campaign = initialize(tmp_path)
    worktree = campaign / "worktree"
    metadata = json.loads((campaign / "campaign.json").read_text(encoding="utf-8"))

    assert worktree.is_dir()
    assert (worktree / ".git").is_dir()
    assert git(worktree, "status", "--porcelain") == ""
    assert git(worktree, "remote") == ""
    assert git(worktree, "branch", "--show-current") == "forge-built"
    assert git(worktree, "log", "-1", "--pretty=%s") == (
        "chore: initialize canonical ForgeBench baseline"
    )
    assert metadata["baseline_commit"] == git(worktree, "rev-parse", "HEAD")
    assert metadata["baseline_tree_sha256"] == canonical_tree_digest(worktree)
    assert metadata["fixture_tree_sha256"] == canonical_tree_digest(FIXTURE)
    assert metadata["worktree"] == "worktree"
    assert metadata["ledger"] == "ledger.jsonl"
    assert metadata["provider"] == "deepseek"
    assert metadata["model"] == "deepseek-v4-flash"
    assert metadata["transport"] == "anthropic-compatible"
    assert metadata["controller_version"] == "forge-eval-controller/1"
    assert metadata["runtime_version"] == "forge-runtime/1"
    assert metadata["controller_interventions"] == [
        {
            "sequence": 0,
            "kind": "campaign_init",
            "reason": "Initialize the locked canonical ForgeBench baseline.",
            "invalidates_campaign": False,
        }
    ]
    assert str(tmp_path) not in json.dumps(metadata)
    assert stat.S_IMODE((campaign / "campaign.json").stat().st_mode) == 0o600
    assert stat.S_IMODE((campaign / "ledger.jsonl").stat().st_mode) == 0o600
    assert (campaign / "ledger.jsonl").read_bytes() == b""
    assert {path.name for path in (campaign / "artifacts").iterdir()} == {
        "traces",
        "reports",
        "patches",
        "verification",
    }
    assert {path.name for path in (campaign / "locks").iterdir()} == {
        "fixture-lock.json",
        "dataset-lock.json",
        "harness-profile.json",
    }
    assert not (FIXTURE / ".git").exists()


def test_initialize_rejects_fixture_content_drift_without_partial_campaign(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    shutil.copytree(FIXTURE, fixture)
    (fixture / "README.md").write_text("tampered\n", encoding="utf-8")

    with pytest.raises(ValueError, match="fixture lock"):
        initialize(tmp_path, fixture=fixture)

    assert not (tmp_path / "campaign").exists()


@pytest.mark.parametrize("generated_name", [".git", "node_modules", "__pycache__"])
def test_initialize_rejects_generated_or_git_state(
    tmp_path: Path,
    generated_name: str,
) -> None:
    fixture = tmp_path / "fixture"
    shutil.copytree(FIXTURE, fixture)
    (fixture / generated_name).mkdir()

    with pytest.raises(ValueError, match="generated state"):
        initialize(tmp_path, fixture=fixture)

    assert not (tmp_path / "campaign").exists()


def test_initialize_rejects_dataset_lock_drift(tmp_path: Path) -> None:
    lock = tmp_path / "dataset-lock.json"
    payload = json.loads(DATASET_LOCK.read_text(encoding="utf-8"))
    payload["holdout"]["count"] = 19
    lock.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(ValueError, match="dataset lock"):
        initialize(tmp_path, dataset_lock=lock)

    assert not (tmp_path / "campaign").exists()


def test_initialize_rejects_secret_bearing_harness_profile(tmp_path: Path) -> None:
    profile = tmp_path / "harness-profile.json"
    write_harness_profile(profile, secret=True)

    with pytest.raises(ValueError, match="secret"):
        initialize(tmp_path, harness_profile=profile)

    assert not (tmp_path / "campaign").exists()


def acceptance_task(task_id: str = "forgebench-dev-domain-normalize-title"):
    tasks = load_cases(FORGEBENCH_ROOT / "cases" / "development")
    return next(task for task in tasks if task.id == task_id)


def write_acceptance_artifacts(
    campaign: Path,
    output: Path,
    *,
    run_id: str = "run-accept-1",
    task=None,
    changes: dict[str, str] | None = None,
    replacements: dict[str, tuple[str, str]] | None = None,
) -> tuple[Path, Path, Path, dict, dict]:
    task = task or acceptance_task()
    worktree = campaign / "worktree"
    metadata = json.loads((campaign / "campaign.json").read_text(encoding="utf-8"))
    changes = changes or {
        "packages/domain/src/task-title.ts": (
            '\nexport const NORMALIZATION_PROFILE = "unicode-whitespace-v1";\n'
        )
    }
    originals = {
        relative_path: (worktree / relative_path).read_text(encoding="utf-8")
        for relative_path in changes
    }
    replacements = replacements or {}
    for relative_path, addition in changes.items():
        updated = originals[relative_path] + addition
        if relative_path in replacements:
            old, new = replacements[relative_path]
            assert old in updated
            updated = updated.replace(old, new)
        (worktree / relative_path).write_text(
            updated,
            encoding="utf-8",
        )
    patch_parts = []
    for relative_path in changes:
        completed = subprocess.run(
            ["git", "diff", "--", relative_path],
            cwd=worktree,
            text=True,
            capture_output=True,
            check=False,
        )
        assert completed.returncode == 0, completed.stderr
        patch_parts.append(completed.stdout)
    patch = "\n".join(patch_parts)
    for relative_path, original in originals.items():
        (worktree / relative_path).write_text(original, encoding="utf-8")
    assert git(worktree, "status", "--porcelain") == ""
    changed_files = list(changes)

    now = datetime(2026, 8, 7, 10, 0, 0, tzinfo=UTC)
    trace = AgentTrace(
        task_id=task.id,
        user_prompt=task.prompt,
        model="deepseek-v4-flash",
        provider=EvalProvider.FORGE,
        file_diffs=[
            FileDiff(
                path=relative_path,
                diff=patch_part,
            )
            for relative_path, patch_part in zip(changed_files, patch_parts, strict=True)
        ],
        changed_files=changed_files,
        expected_files_changed=task.expected_files_changed,
        forbidden_files_changed=task.forbidden_files_changed,
        required_scores=task.required_scores,
        final_answer="Implemented and verified.",
        verification_result=VerificationResult(
            command=task.validation_commands[0],
            passed=True,
            stdout="passed",
            stderr="",
            exit_code=0,
            duration_ms=10,
        ),
        workspace_observation=WorkspaceObservation(
            available=True,
            source="filesystem_snapshot",
            changed_files=changed_files,
            modified_files=changed_files,
            reported_changed_files=changed_files,
        ),
        sandbox_scrub=LeakageCheck(ok=True),
        patch_replay=WorkspaceCheck(ok=True),
        started_at=now,
        ended_at=now,
        duration_ms=10,
        cost_usd=0.01,
    )
    report = build_report([trace]).model_copy(
        update={
            "trust_result": TrustGateResult(
                status=TrustStatus.TRUSTED,
                trusted=True,
            )
        }
    )
    dataset_lock = json.loads((campaign / "locks/dataset-lock.json").read_text(encoding="utf-8"))
    identity = {
        "schema_version": 1,
        "run_id": run_id,
        "provider": metadata["provider"],
        "model": metadata["model"],
        "transport": metadata["transport"],
        "input_commit": git(worktree, "rev-parse", "HEAD"),
        "dataset_fingerprint": dataset_lock["development"]["fingerprint"],
        "harness_fingerprint": metadata["harness_profile_sha256"],
        "cost_cny": "0.25",
    }
    trace_payload = {**identity, "trace": trace.model_dump(mode="json")}
    report_payload = {
        **{key: value for key, value in identity.items() if key != "input_commit"},
        "report": report.model_dump(mode="json"),
    }
    output.mkdir(parents=True, exist_ok=True)
    trace_path = output / f"{run_id}.trace.json"
    report_path = output / f"{run_id}.report.json"
    patch_path = output / f"{run_id}.patch"
    trace_path.write_text(json.dumps(trace_payload, sort_keys=True), encoding="utf-8")
    report_path.write_text(json.dumps(report_payload, sort_keys=True), encoding="utf-8")
    patch_path.write_text(patch, encoding="utf-8")
    return trace_path, report_path, patch_path, trace_payload, report_payload


def accept(
    campaign: Path,
    trace_path: Path,
    report_path: Path,
    patch_path: Path,
    **kwargs,
):
    task = kwargs.pop("task", acceptance_task())
    return accept_campaign_patch(
        campaign,
        task=task,
        trace_path=trace_path,
        report_path=report_path,
        patch_path=patch_path,
        expected_trace_sha256=kwargs.pop(
            "expected_trace_sha256", sha256_bytes(trace_path.read_bytes())
        ),
        expected_report_sha256=kwargs.pop(
            "expected_report_sha256", sha256_bytes(report_path.read_bytes())
        ),
        expected_patch_sha256=kwargs.pop(
            "expected_patch_sha256", sha256_bytes(patch_path.read_bytes())
        ),
        **kwargs,
    )


@pytest.mark.parametrize(
    "rejection",
    [
        "trust",
        "task_failed",
        "observation_unavailable",
        "observation_mismatch",
        "scope",
        "patch_replay",
        "model",
        "dataset",
        "harness",
        "input_commit",
        "patch_content",
        "trace_hash",
        "report_identity",
    ],
)
def test_accept_rejection_matrix_is_side_effect_free(
    tmp_path: Path,
    monkeypatch,
    rejection: str,
) -> None:
    campaign = initialize(tmp_path)
    artifacts_dir = tmp_path / "inputs"
    trace_path, report_path, patch_path, trace_payload, report_payload = write_acceptance_artifacts(
        campaign, artifacts_dir
    )
    if rejection == "trust":
        report_payload["report"]["trust_result"] = TrustGateResult().model_dump(mode="json")
    elif rejection == "task_failed":
        report_payload["report"]["tasks"][0]["passed"] = False
    elif rejection == "observation_unavailable":
        trace_payload["trace"]["workspace_observation"]["available"] = False
    elif rejection == "observation_mismatch":
        trace_payload["trace"]["workspace_observation"]["mismatch_files"] = ["wrong.ts"]
    elif rejection == "scope":
        trace_payload["trace"]["scope_violations"] = ["package.json"]
    elif rejection == "patch_replay":
        trace_payload["trace"]["patch_replay"]["ok"] = False
    elif rejection == "model":
        trace_payload["model"] = "different-model"
    elif rejection == "dataset":
        trace_payload["dataset_fingerprint"] = "d" * 64
    elif rejection == "harness":
        report_payload["harness_fingerprint"] = "e" * 64
    elif rejection == "input_commit":
        trace_payload["input_commit"] = "f" * 40
    elif rejection == "patch_content":
        patch_path.write_text("not the trace patch\n", encoding="utf-8")
    elif rejection == "report_identity":
        report_payload["run_id"] = "another-run"
    trace_path.write_text(json.dumps(trace_payload, sort_keys=True), encoding="utf-8")
    report_path.write_text(json.dumps(report_payload, sort_keys=True), encoding="utf-8")

    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    head = git(campaign / "worktree", "rev-parse", "HEAD")
    ledger_before = (campaign / "ledger.jsonl").read_bytes()
    expected_trace = (
        "0" * 64 if rejection == "trace_hash" else sha256_bytes(trace_path.read_bytes())
    )

    with pytest.raises(ValueError):
        accept(
            campaign,
            trace_path,
            report_path,
            patch_path,
            expected_trace_sha256=expected_trace,
        )

    assert git(campaign / "worktree", "rev-parse", "HEAD") == head
    assert git(campaign / "worktree", "status", "--porcelain") == ""
    assert (campaign / "ledger.jsonl").read_bytes() == ledger_before
    assert not (campaign / "pending-accept.json").exists()


def test_accept_rejects_failed_independent_verification_and_dirty_controller_edit(
    tmp_path: Path,
    monkeypatch,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {
            "passed": False,
            "checks": [{"kind": "hidden", "exit_code": 1}],
        },
    )

    with pytest.raises(ValueError, match="verification"):
        accept(campaign, trace_path, report_path, patch_path)
    assert git(campaign / "worktree", "status", "--porcelain") == ""
    assert (campaign / "ledger.jsonl").read_bytes() == b""

    readme = campaign / "worktree/README.md"
    original = readme.read_text(encoding="utf-8")
    readme.write_text(original + "\ncontroller edit\n", encoding="utf-8")
    with pytest.raises(ValueError, match="clean"):
        accept(campaign, trace_path, report_path, patch_path)
    readme.write_text(original, encoding="utf-8")
    assert git(campaign / "worktree", "status", "--porcelain") == ""


def test_accept_rejects_unfrozen_task_definition_and_unsafe_run_id(
    tmp_path: Path,
    monkeypatch,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, trace_payload, report_payload = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    forged_task = acceptance_task().model_copy(update={"validation_commands": ["true"]})
    with pytest.raises(ValueError, match="frozen"):
        accept_campaign_patch(
            campaign,
            task=forged_task,
            trace_path=trace_path,
            report_path=report_path,
            patch_path=patch_path,
            expected_trace_sha256=sha256_bytes(trace_path.read_bytes()),
            expected_report_sha256=sha256_bytes(report_path.read_bytes()),
            expected_patch_sha256=sha256_bytes(patch_path.read_bytes()),
        )

    trace_payload["run_id"] = "../escape"
    report_payload["run_id"] = "../escape"
    trace_path.write_text(json.dumps(trace_payload, sort_keys=True), encoding="utf-8")
    report_path.write_text(json.dumps(report_payload, sort_keys=True), encoding="utf-8")
    with pytest.raises(ValueError, match="identifier"):
        accept(campaign, trace_path, report_path, patch_path)
    assert not (campaign / "artifacts/escape").exists()
    assert git(campaign / "worktree", "status", "--porcelain") == ""


def test_accept_refuses_a_concurrent_campaign_mutation(
    tmp_path: Path,
    monkeypatch,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    lock_descriptor = os.open(
        campaign / ".campaign.lock",
        os.O_CREAT | os.O_RDWR,
        0o600,
    )
    try:
        fcntl.flock(lock_descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with pytest.raises(ValueError, match="locked"):
            accept(campaign, trace_path, report_path, patch_path)
    finally:
        fcntl.flock(lock_descriptor, fcntl.LOCK_UN)
        os.close(lock_descriptor)
    assert git(campaign / "worktree", "status", "--porcelain") == ""
    assert (campaign / "ledger.jsonl").read_bytes() == b""


def test_accept_commits_verified_patch_and_appends_provenance(
    tmp_path: Path,
    monkeypatch,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {
            "passed": True,
            "checks": [
                {"kind": "visible", "exit_code": 0},
                {"kind": "hidden", "exit_code": 0},
                {"kind": "full", "exit_code": 0},
            ],
        },
    )

    result = accept(campaign, trace_path, report_path, patch_path)
    worktree = campaign / "worktree"
    ledger = verify_ledger(campaign / "ledger.jsonl", campaign / "artifacts")
    message = git(worktree, "log", "-1", "--pretty=%B")

    assert result["task_id"] == acceptance_task().id
    assert result["run_id"] == "run-accept-1"
    assert result["output_commit"] == git(worktree, "rev-parse", "HEAD")
    assert "Forge-Run-Id: run-accept-1" in message
    assert f"Forge-Task-Id: {acceptance_task().id}" in message
    assert "Forge-Trace-SHA256:" in message
    assert "Forge-Patch-SHA256:" in message
    assert ledger.ok is True
    assert ledger.entry_count == 1
    assert git(worktree, "status", "--porcelain") == ""
    assert not (campaign / "pending-accept.json").exists()
    for kind, suffix in (
        ("traces", ".json"),
        ("reports", ".json"),
        ("patches", ".patch"),
        ("verification", ".json"),
    ):
        assert (campaign / "artifacts" / kind / f"run-accept-1{suffix}").is_file()

    with pytest.raises(ValueError):
        accept(campaign, trace_path, report_path, patch_path)

    duplicate_trace, duplicate_report, duplicate_patch, _trace, _report = (
        write_acceptance_artifacts(
            campaign,
            tmp_path / "duplicate-inputs",
            run_id="run-duplicate-task",
        )
    )
    accepted_head = git(worktree, "rev-parse", "HEAD")
    with pytest.raises(ValueError, match="task id"):
        accept(
            campaign,
            duplicate_trace,
            duplicate_report,
            duplicate_patch,
        )
    assert git(worktree, "rev-parse", "HEAD") == accepted_head
    assert git(worktree, "status", "--porcelain") == ""
    assert verify_ledger(campaign / "ledger.jsonl", campaign / "artifacts").entry_count == 1


def test_accept_two_file_patch_with_real_independent_verification(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    task = acceptance_task("forgebench-dev-admin-bulk-status")
    changes = {
        "apps/admin-console/src/bulk-actions.ts": (
            "\nexport const FORGE_ADMIN_ACCEPTANCE = true;\n"
        ),
        "packages/domain/src/task.ts": ("\nexport const FORGE_DOMAIN_ACCEPTANCE = true;\n"),
    }
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign,
        tmp_path / "inputs",
        run_id="run-two-file",
        task=task,
        changes=changes,
        replacements={
            "packages/domain/src/task.ts": (
                'done: ["in_progress"],',
                "done: [],",
            )
        },
    )

    result = accept(
        campaign,
        trace_path,
        report_path,
        patch_path,
        task=task,
    )

    verification = json.loads(
        (campaign / "artifacts/verification/run-two-file.json").read_text(encoding="utf-8")
    )
    assert result["status"] == "accepted"
    assert {check["kind"] for check in verification["checks"]} == {
        "visible",
        "hidden",
        "full",
    }
    assert verification["passed"] is True
    assert git(campaign / "worktree", "status", "--porcelain") == ""


@pytest.mark.parametrize(
    ("crash_at", "expected_recovery"),
    [
        ("after_pending_before_commit", "rolled_back"),
        ("after_staging", "rolled_back"),
        ("after_commit", "completed"),
        ("after_artifact_publish", "completed"),
        ("after_ledger_append", "completed"),
    ],
)
def test_recover_pending_accept_is_exact_and_idempotent(
    tmp_path: Path,
    monkeypatch,
    crash_at: str,
    expected_recovery: str,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    baseline = git(campaign / "worktree", "rev-parse", "HEAD")

    with pytest.raises(CampaignCrashInjected):
        accept(
            campaign,
            trace_path,
            report_path,
            patch_path,
            crash_at=crash_at,
        )
    assert (campaign / "pending-accept.json").is_file()

    recovered = recover_pending_accept(campaign)

    assert recovered["status"] == expected_recovery
    assert not (campaign / "pending-accept.json").exists()
    assert git(campaign / "worktree", "status", "--porcelain") == ""
    if expected_recovery == "rolled_back":
        assert git(campaign / "worktree", "rev-parse", "HEAD") == baseline
        assert verify_ledger(campaign / "ledger.jsonl", campaign / "artifacts").entry_count == 0
    else:
        assert git(campaign / "worktree", "rev-parse", "HEAD") != baseline
        assert verify_ledger(campaign / "ledger.jsonl", campaign / "artifacts").entry_count == 1

    assert recover_pending_accept(campaign)["status"] == "nothing_pending"


@pytest.mark.parametrize(
    "crash_at",
    ["after_patch", "after_verification", "after_artifact_prepare"],
)
def test_accept_rolls_back_exactly_when_crashing_before_pending_record(
    tmp_path: Path,
    monkeypatch,
    crash_at: str,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    baseline = git(campaign / "worktree", "rev-parse", "HEAD")

    with pytest.raises(CampaignCrashInjected):
        accept(
            campaign,
            trace_path,
            report_path,
            patch_path,
            crash_at=crash_at,
        )

    assert git(campaign / "worktree", "rev-parse", "HEAD") == baseline
    assert git(campaign / "worktree", "status", "--porcelain") == ""
    assert (campaign / "ledger.jsonl").read_bytes() == b""
    assert not (campaign / "pending-accept.json").exists()
    assert not (campaign / "artifacts/.pending").exists()


def test_recover_resumes_after_partial_artifact_publication(
    tmp_path: Path,
    monkeypatch,
) -> None:
    campaign = initialize(tmp_path)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    monkeypatch.setattr(
        "app.campaign._run_accept_verification",
        lambda *_args, **_kwargs: {"passed": True, "checks": []},
    )
    with pytest.raises(CampaignCrashInjected):
        accept(
            campaign,
            trace_path,
            report_path,
            patch_path,
            crash_at="after_commit",
        )
    pending = json.loads((campaign / "pending-accept.json").read_text(encoding="utf-8"))
    trace_spec = pending["temporary_artifacts"]["traces"]
    os.replace(
        campaign / trace_spec["temporary_path"],
        campaign / trace_spec["final_path"],
    )

    recovered = recover_pending_accept(campaign)

    assert recovered["status"] == "completed"
    assert verify_ledger(campaign / "ledger.jsonl", campaign / "artifacts").entry_count == 1
    assert not (campaign / "pending-accept.json").exists()
