import json
import stat
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from pydantic import ValidationError

from app.baseline import (
    BaselineCrashInjected,
    initialize_baseline_campaign,
    recover_baseline_campaign,
    run_baseline_campaign,
    verify_baseline_campaign,
)
from app.baseline_models import (
    BaselineAttemptArtifact,
    BaselineBudgetEntry,
    BaselinePendingAttempt,
)
from app.cases import load_cases
from app.harness_profiles import (
    harness_profile_bytes,
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.models import (
    AgentTrace,
    EvalProvider,
    EvaluationTask,
    ForgeRunEvidence,
    LeakageCheck,
    TrustGateResult,
    TrustStatus,
    VerificationResult,
    WorkspaceCheck,
    WorkspaceObservation,
)
from app.provenance import canonical_json_bytes, sha256_bytes

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
DEVELOPMENT_CASES = FORGEBENCH_ROOT / "cases" / "development"
HOLDOUT_CASES = FORGEBENCH_ROOT / "cases" / "holdout"
DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"
PROFILE_PATH = (
    FORGEBENCH_ROOT
    / "profiles"
    / "deepseek-v4-flash-anthropic-baseline-v1.json"
)


def provider_event(
    *,
    input_tokens: int | None = 1_000,
    cache_read_tokens: int | None = 250,
    output_tokens: int | None = 100,
    provider_id: str = "deepseek",
    model: str = "deepseek-v4-flash",
    reason: str = "provider_reported",
) -> dict[str, Any]:
    return {
        "provider_id": provider_id,
        "model": model,
        "input_tokens": input_tokens,
        "cache_read_tokens": cache_read_tokens,
        "output_tokens": output_tokens,
        "reasoning_tokens": None,
        "source": "deepseek",
        "reason": reason,
    }


def make_trace(
    task: EvaluationTask,
    *,
    event: dict[str, Any] | None = None,
    identity_matches: bool = True,
    harness_fingerprint_value: str = "0" * 64,
    workspace_available: bool = True,
    patch_replay_ok: bool = True,
) -> AgentTrace:
    now = datetime.now(UTC)
    usage_event = event or provider_event()
    provider = (
        "deepseek" if identity_matches else "unexpected-provider"
    )
    model = (
        "deepseek-v4-flash" if identity_matches else "unexpected-model"
    )
    return AgentTrace(
        task_id=task.id,
        user_prompt=task.prompt,
        model="deepseek-v4-flash-anthropic-baseline-v1",
        provider=EvalProvider.FORGE,
        required_scores=task.required_scores,
        raw_events=[
            {
                "event_type": "eval_agent_identity",
                "provider": provider,
                "model": model,
                "transport": "anthropic_messages",
                "harness_fingerprint": harness_fingerprint_value,
                "identity_matches_profile": identity_matches,
            }
        ],
        changed_files=[],
        final_answer="Done.",
        verification_result=VerificationResult(
            command="python3 tools/release/verify.py",
            passed=True,
            exit_code=0,
        ),
        forge_run_evidence=ForgeRunEvidence(
            provider_usage={"events": [usage_event]}
        ),
        workspace_observation=WorkspaceObservation(
            available=workspace_available,
            source="fake_runner",
            changed_files=[],
            reported_changed_files=[],
        ),
        sandbox_scrub=LeakageCheck(ok=True),
        patch_replay=WorkspaceCheck(ok=patch_replay_ok),
        started_at=now,
        ended_at=now,
        duration_ms=0,
    )


class FakeRunner:
    def __init__(
        self,
        *,
        campaign_root: Path | None = None,
        event: dict[str, Any] | None = None,
        identity_matches: bool = True,
        workspace_available: bool = True,
        patch_replay_ok: bool = True,
        fail_after_pending: bool = False,
    ) -> None:
        self.campaign_root = campaign_root
        self.event = event
        self.identity_matches = identity_matches
        self.workspace_available = workspace_available
        self.patch_replay_ok = patch_replay_ok
        self.fail_after_pending = fail_after_pending
        self.calls: list[str] = []

    def run_task(self, task: EvaluationTask) -> AgentTrace:
        self.calls.append(task.id)
        harness_fingerprint_value = "0" * 64
        if self.campaign_root is not None:
            pending = list((self.campaign_root / "pending").glob("*.json"))
            assert len(pending) == 1
            assert json.loads(pending[0].read_text())["state"] == "provider_call_pending"
            harness_fingerprint_value = json.loads(
                (self.campaign_root / "campaign.json").read_text()
            )["harness_fingerprint"]
        if self.fail_after_pending:
            raise RuntimeError("simulated provider interruption")
        return make_trace(
            task,
            event=self.event,
            identity_matches=self.identity_matches,
            harness_fingerprint_value=harness_fingerprint_value,
            workspace_available=self.workspace_available,
            patch_replay_ok=self.patch_replay_ok,
        )


def write_profile(
    tmp_path: Path,
    *,
    per_task_cost: str = "1",
    campaign_budget: str = "8",
) -> Path:
    profile = load_harness_profile(PROFILE_PATH).model_copy(
        update={
            "max_cost_cny_per_task": Decimal(per_task_cost),
            "campaign_budget_cny": Decimal(campaign_budget),
        }
    )
    path = tmp_path / "profile.json"
    path.write_bytes(harness_profile_bytes(profile) + b"\n")
    return path


def initialize(
    tmp_path: Path,
    *,
    profile_path: Path = PROFILE_PATH,
    cases_path: Path = DEVELOPMENT_CASES,
) -> Path:
    campaign = tmp_path / "baseline"
    initialize_baseline_campaign(
        campaign,
        campaign_id="h1-test",
        repository_root=REPOSITORY_ROOT,
        cases_path=cases_path,
        dataset_lock_path=DATASET_LOCK,
        profile_path=profile_path,
    )
    return campaign


def campaign_kwargs(
    campaign: Path,
    *,
    profile_path: Path = PROFILE_PATH,
) -> dict[str, Path]:
    return {
        "campaign_root": campaign,
        "repository_root": REPOSITORY_ROOT,
        "cases_path": DEVELOPMENT_CASES,
        "dataset_lock_path": DATASET_LOCK,
        "profile_path": profile_path,
    }


def test_baseline_models_reject_extra_fields_blank_ids_and_digest_drift() -> None:
    profile = load_harness_profile(PROFILE_PATH)
    task = load_cases(DEVELOPMENT_CASES)[0]
    trace = make_trace(task)
    trace_sha256 = sha256_bytes(canonical_json_bytes(trace.model_dump(mode="json")))
    common = {
        "attempt_id": "h1-test-r01-task",
        "task_id": task.id,
        "repetition": 1,
        "source_commit": "a" * 40,
        "dirty_state_sha256": "b" * 64,
        "dataset_fingerprint": "c" * 64,
        "harness_fingerprint": harness_profile_fingerprint(profile),
    }

    with pytest.raises(ValidationError):
        BaselinePendingAttempt.model_validate(
            {
                **common,
                "reserved_cost_cny": "1",
                "started_at": datetime.now(UTC).isoformat(),
                "unexpected": True,
            }
        )
    with pytest.raises(ValidationError):
        BaselinePendingAttempt.model_validate(
            {
                **common,
                "attempt_id": " ",
                "reserved_cost_cny": "1",
                "started_at": datetime.now(UTC).isoformat(),
            }
        )
    with pytest.raises(ValidationError, match="trace digest"):
        BaselineAttemptArtifact(
            **common,
            profile=profile,
            trace_sha256="d" * 64,
            trace=trace,
            usage={
                "status": "unknown",
                "provider_event_count": 0,
                "blockers": ["test"],
            },
            trust_result=TrustGateResult(
                status=TrustStatus.UNKNOWN,
                trusted=False,
                blockers=["test"],
            ),
            primary_failure_category="none",
            terminal=True,
        )
    assert trace_sha256 != "d" * 64


def test_budget_entry_requires_valid_first_link_and_cumulative_cost() -> None:
    with pytest.raises(ValidationError, match="first budget entry"):
        BaselineBudgetEntry(
            sequence=1,
            previous_entry_sha256="a" * 64,
            attempt_id="attempt-1",
            status="known",
            cost_cny="0.1",
            cumulative_cost_cny="0.1",
            artifact_sha256="b" * 64,
        )
    with pytest.raises(ValidationError, match="cumulative"):
        BaselineBudgetEntry(
            sequence=1,
            previous_entry_sha256=None,
            attempt_id="attempt-1",
            status="known",
            cost_cny="1",
            cumulative_cost_cny="0.9",
            artifact_sha256="b" * 64,
        )


def test_initialize_freezes_exact_development_catalog_and_source(tmp_path: Path) -> None:
    campaign = initialize(tmp_path)
    metadata = json.loads((campaign / "campaign.json").read_text())
    lock = json.loads(DATASET_LOCK.read_text())

    assert metadata["task_ids"] == lock["development"]["ids"]
    assert metadata["dataset_fingerprint"] == lock["development"]["fingerprint"]
    assert metadata["repetitions"] == 3
    assert metadata["budget_cny"] == "8"
    assert metadata["source_commit"]
    assert metadata["dirty_state_sha256"]
    assert metadata["profile"]["dataset_split"] == "development"
    assert str(tmp_path) not in json.dumps(metadata)
    assert (campaign / "budget.jsonl").read_bytes() == b""
    assert stat.S_IMODE((campaign / "budget.jsonl").stat().st_mode) == 0o600
    assert verify_baseline_campaign(**campaign_kwargs(campaign)).ok is True


def test_initialize_rejects_holdout_cases_without_partial_campaign(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="development"):
        initialize(tmp_path, cases_path=HOLDOUT_CASES)

    assert not (tmp_path / "baseline").exists()


def test_paid_attempt_is_pending_before_call_then_published_and_resumable(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    first_runner = FakeRunner(campaign_root=campaign)

    first = run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=first_runner,
        max_attempts=1,
    )

    assert first.status == "paused"
    assert first.calls_started == 1
    assert len(first_runner.calls) == 1
    assert list((campaign / "pending").iterdir()) == []
    assert len(list((campaign / "attempts").glob("*.json"))) == 1
    assert (campaign / "budget.jsonl").read_bytes().endswith(b"\n")
    verification = verify_baseline_campaign(**campaign_kwargs(campaign))
    assert verification.ok is True
    assert verification.attempt_count == 1
    assert verification.spent_cny == Decimal("0.000955")

    second_runner = FakeRunner(campaign_root=campaign)
    second = run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=second_runner,
        max_attempts=1,
    )
    assert second.status == "paused"
    assert second.calls_started == 1
    assert second_runner.calls[0] != first_runner.calls[0]
    assert verify_baseline_campaign(**campaign_kwargs(campaign)).attempt_count == 2


def test_budget_reservation_prevents_call_above_campaign_cap(tmp_path: Path) -> None:
    profile_path = write_profile(tmp_path, campaign_budget="1")
    campaign = initialize(tmp_path / "campaign-root", profile_path=profile_path)
    first_runner = FakeRunner(campaign_root=campaign)
    run_baseline_campaign(
        **campaign_kwargs(campaign, profile_path=profile_path),
        runner=first_runner,
        max_attempts=1,
    )
    second_runner = FakeRunner(campaign_root=campaign)

    result = run_baseline_campaign(
        **campaign_kwargs(campaign, profile_path=profile_path),
        runner=second_runner,
    )

    assert result.status == "blocked"
    assert result.blockers == ["campaign_budget_reservation_exceeded"]
    assert second_runner.calls == []


def test_interrupted_paid_attempt_blocks_resume_and_recovery_never_reissues(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    interrupted_runner = FakeRunner(campaign_root=campaign, fail_after_pending=True)

    interrupted = run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=interrupted_runner,
        max_attempts=1,
    )
    assert interrupted.status == "blocked"
    assert interrupted.blockers == ["interrupted_paid_attempt_unknown_cost"]
    assert len(list((campaign / "pending").glob("*.json"))) == 1

    forbidden_runner = FakeRunner(campaign_root=campaign)
    resumed = run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=forbidden_runner,
    )
    assert resumed.blockers == ["interrupted_paid_attempt_unknown_cost"]
    assert forbidden_runner.calls == []

    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))
    assert recovered.status == "blocked"
    assert recovered.blockers == ["interrupted_paid_attempt_unknown_cost"]
    assert list((campaign / "pending").iterdir()) == []
    assert forbidden_runner.calls == []
    verification = verify_baseline_campaign(**campaign_kwargs(campaign))
    assert "interrupted_paid_attempt_unknown_cost" in verification.blockers
    assert verification.spent_cny == Decimal("1")


@pytest.mark.parametrize(
    ("runner_kwargs", "expected_blocker"),
    [
        (
            {
                "event": provider_event(
                    input_tokens=None,
                    cache_read_tokens=None,
                    output_tokens=None,
                    reason="provider_omitted",
                )
            },
            "provider_usage_unknown",
        ),
        ({"identity_matches": False}, "agent_identity_mismatch"),
        ({"workspace_available": False}, "trust_result_untrusted"),
        ({"patch_replay_ok": False}, "trust_result_untrusted"),
        (
            {
                "event": provider_event(
                    input_tokens=1_000_001,
                    cache_read_tokens=0,
                    output_tokens=0,
                )
            },
            "per_task_cost_cap_exceeded",
        ),
    ],
)
def test_fail_closed_attempt_stops_before_next_provider_call(
    tmp_path: Path,
    runner_kwargs: dict[str, Any],
    expected_blocker: str,
) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign, **runner_kwargs)

    result = run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=runner,
    )

    assert result.status == "blocked"
    assert expected_blocker in result.blockers
    assert len(runner.calls) == 1


def test_recovery_finishes_ledger_for_already_published_artifact(tmp_path: Path) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)

    with pytest.raises(BaselineCrashInjected, match="after_artifact_publish"):
        run_baseline_campaign(
            **campaign_kwargs(campaign),
            runner=runner,
            max_attempts=1,
            crash_at="after_artifact_publish",
        )

    assert len(list((campaign / "pending").glob("*.json"))) == 1
    assert len(list((campaign / "attempts").glob("*.json"))) == 1
    assert (campaign / "budget.jsonl").read_bytes() == b""

    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))
    assert recovered.status == "paused"
    assert recovered.blockers == []
    assert list((campaign / "pending").iterdir()) == []
    assert verify_baseline_campaign(**campaign_kwargs(campaign)).ok is True


def test_recovery_is_idempotent_after_ledger_append_before_pending_removal(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)

    with pytest.raises(BaselineCrashInjected, match="after_ledger_append"):
        run_baseline_campaign(
            **campaign_kwargs(campaign),
            runner=runner,
            max_attempts=1,
            crash_at="after_ledger_append",
        )

    ledger_before = (campaign / "budget.jsonl").read_bytes()
    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))

    assert recovered.status == "paused"
    assert recovered.blockers == []
    assert (campaign / "budget.jsonl").read_bytes() == ledger_before
    assert list((campaign / "pending").iterdir()) == []


def test_recovery_refuses_tampered_published_artifact(tmp_path: Path) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)
    with pytest.raises(BaselineCrashInjected):
        run_baseline_campaign(
            **campaign_kwargs(campaign),
            runner=runner,
            max_attempts=1,
            crash_at="after_artifact_publish",
        )
    artifact_path = next((campaign / "attempts").glob("*.json"))
    payload = json.loads(artifact_path.read_text())
    payload["trace"]["final_answer"] = "tampered"
    artifact_path.write_bytes(canonical_json_bytes(payload) + b"\n")

    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))

    assert recovered.status == "blocked"
    assert recovered.blockers == ["campaign_state_invalid"]
    assert len(list((campaign / "pending").glob("*.json"))) == 1
    assert (campaign / "budget.jsonl").read_bytes() == b""


def test_recovery_validates_existing_ledger_before_removing_pending(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)
    with pytest.raises(BaselineCrashInjected):
        run_baseline_campaign(
            **campaign_kwargs(campaign),
            runner=runner,
            max_attempts=1,
            crash_at="after_ledger_append",
        )
    ledger_path = campaign / "budget.jsonl"
    entry = json.loads(ledger_path.read_text())
    entry["cost_cny"] = "0.5"
    entry["cumulative_cost_cny"] = "0.5"
    ledger_path.write_bytes(canonical_json_bytes(entry) + b"\n")

    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))

    assert recovered.status == "blocked"
    assert recovered.blockers == ["campaign_state_invalid"]
    assert len(list((campaign / "pending").glob("*.json"))) == 1


def test_recovery_accounts_complete_unknown_usage_artifact_then_stops(
    tmp_path: Path,
) -> None:
    campaign = initialize(tmp_path)
    runner = FakeRunner(
        campaign_root=campaign,
        event=provider_event(
            input_tokens=None,
            cache_read_tokens=None,
            output_tokens=None,
            reason="provider_omitted",
        ),
    )
    with pytest.raises(BaselineCrashInjected):
        run_baseline_campaign(
            **campaign_kwargs(campaign),
            runner=runner,
            max_attempts=1,
            crash_at="after_artifact_publish",
        )

    recovered = recover_baseline_campaign(**campaign_kwargs(campaign))

    assert recovered.status == "blocked"
    assert "provider_usage_unknown" in recovered.blockers
    assert recovered.spent_cny == Decimal("1")
    assert list((campaign / "pending").iterdir()) == []


@pytest.mark.parametrize(
    "target",
    ["profile", "dataset_lock", "artifact", "ledger", "source_commit", "dirty_digest"],
)
def test_campaign_tampering_is_detected(tmp_path: Path, target: str) -> None:
    campaign = initialize(tmp_path)
    run_baseline_campaign(
        **campaign_kwargs(campaign),
        runner=FakeRunner(campaign_root=campaign),
        max_attempts=1,
    )

    if target == "profile":
        path = campaign / "locks" / "profile.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif target == "dataset_lock":
        path = campaign / "locks" / "dataset-lock.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif target == "artifact":
        path = next((campaign / "attempts").glob("*.json"))
        payload = json.loads(path.read_text())
        payload["trace"]["final_answer"] = "tampered"
        path.write_bytes(canonical_json_bytes(payload) + b"\n")
    elif target == "ledger":
        path = campaign / "budget.jsonl"
        path.write_bytes(path.read_bytes().replace(b'"sequence":1', b'"sequence":2'))
    else:
        path = campaign / "campaign.json"
        payload = json.loads(path.read_text())
        payload["source_commit" if target == "source_commit" else "dirty_state_sha256"] = (
            "f" * 40 if target == "source_commit" else "f" * 64
        )
        path.write_bytes(canonical_json_bytes(payload) + b"\n")

    verification = verify_baseline_campaign(**campaign_kwargs(campaign))

    assert verification.ok is False
    assert verification.blockers
