import json
from datetime import UTC, datetime

import pytest

from app.execution import EvaluationExecution
from app.execution_identity import default_execution_identity
from app.models import (
    AgentTrace,
    EvalProvider,
    EvalScore,
    EvaluationTask,
    FailureCategory,
    ForgeRunEvidence,
    TrustGateResult,
    TrustStatus,
    VerificationResult,
)
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provider_ab import (
    ProviderABArm,
    ProviderABError,
    ProviderABRunContract,
    _distribution_summary,
    compare_provider_ab,
    load_provider_ab_artifact,
    provider_ab_main,
    provider_ab_verify_main,
    run_provider_ab,
    write_provider_ab_artifact,
)
from app.reporting import build_report


def _trace(
    *,
    task_id: str,
    provider: str,
    model: str,
    transport: str,
    endpoint_family: str,
    base_url: str,
    passed: bool = True,
    fallback: bool = False,
) -> AgentTrace:
    now = datetime.now(UTC)
    identity = default_execution_identity(
        provider=provider,
        model=model,
        transport=transport,
        base_url=base_url,
    )
    raw_events = [{"event_type": "eval_execution_identity", **identity.model_dump(mode="json")}]
    if fallback:
        raw_events.append(
            {
                "event_type": "eval_provider_fallback",
                "reason_code": "rate_limit",
                "primary_provider": provider,
                "primary_model": model,
                "fallback_provider": "deepseek",
                "fallback_model": "deepseek-v4-flash",
            }
        )
    return AgentTrace(
        task_id=task_id,
        user_prompt="Do the task.",
        model=model,
        provider=EvalProvider.FORGE,
        raw_events=raw_events,
        changed_files=["src/app.py"] if passed else [],
        expected_files_changed=["src/app.py"],
        final_answer="done" if passed else "failed",
        verification_result=VerificationResult(
            command="python -c 'pass'",
            passed=passed,
            exit_code=0 if passed else 1,
        ),
        started_at=now,
        ended_at=now,
        duration_ms=100,
        model_rounds=2,
        input_tokens=100,
        output_tokens=20,
        cost_usd=0.01,
    )


def _arm(
    *,
    arm_id: str,
    task_ids: list[str],
    provider: str,
    model: str,
    transport: str,
    endpoint_family: str,
    base_url: str,
    passed: list[bool] | None = None,
    trusted: bool = True,
    fallback: bool = False,
    dataset_fingerprint: str = "a" * 64,
) -> ProviderABArm:
    traces = tuple(
        _trace(
            task_id=task_id,
            provider=provider,
            model=model,
            transport=transport,
            endpoint_family=endpoint_family,
            base_url=base_url,
            passed=(passed or [True] * len(task_ids))[index],
            fallback=fallback,
        )
        for index, task_id in enumerate(task_ids)
    )
    report = build_report(list(traces))
    if trusted:
        report = report.model_copy(
            update={
                "trust_result": TrustGateResult(
                    status=TrustStatus.TRUSTED,
                    trusted=True,
                )
            }
        )
    else:
        report = report.model_copy(
            update={
                "trust_result": TrustGateResult(
                    status=TrustStatus.UNTRUSTED,
                    trusted=False,
                    blockers=["workspace_untrusted"],
                )
            }
        )
    return ProviderABArm(
        arm_id=arm_id,
        dataset_fingerprint=dataset_fingerprint,
        traces=traces,
        report=report,
    )


def _kimi(task_ids: list[str], **kwargs: object) -> ProviderABArm:
    return _arm(
        arm_id="kimi_k3",
        task_ids=task_ids,
        provider="custom_openai",
        model="k3",
        transport="openai_chat_completions",
        endpoint_family="openai",
        base_url="https://api.kimi.com/coding/v1",
        **kwargs,
    )


def _deepseek(task_ids: list[str], **kwargs: object) -> ProviderABArm:
    return _arm(
        arm_id="deepseek_v4_flash",
        task_ids=task_ids,
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic_messages",
        endpoint_family="anthropic",
        base_url="https://api.deepseek.com/anthropic",
        **kwargs,
    )


def test_provider_ab_pairs_same_task_order_and_reports_quality_delta() -> None:
    control = _deepseek(["task-a", "task-b"])
    candidate = _kimi(["task-a", "task-b"], passed=[True, False])

    comparison = compare_provider_ab(control=control, candidate=candidate)

    assert comparison.paired_attempts == 2
    assert comparison.outcome_changes == {
        "r1:task-a": "success->success",
        "r1:task-b": "success->failure",
    }
    assert comparison.deltas["success_rate"] == -0.5
    # With only two paired attempts the bootstrap interval includes zero, so
    # the gate correctly refuses to overclaim a control win.
    assert comparison.winner == "no_material_difference"
    assert comparison.blockers == []


def test_provider_ab_declares_control_only_when_paired_interval_excludes_zero() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"], passed=[False]),
    )

    assert comparison.winner == "control"
    assert comparison.bootstrap_intervals["success_rate"].high < 0


def test_provider_ab_blocks_candidate_when_harness_evidence_score_regresses() -> None:
    control = _deepseek(["task-a"], passed=[False])
    candidate = _kimi(["task-a"], passed=[True])

    control_trace = control.traces[0].model_copy(
        update={
            "forge_run_evidence": ForgeRunEvidence(
                source="forge_headless_trace",
                session_id="control-session",
                turn_id="control-turn",
                verification={
                    "command": "python -c 'pass'",
                    "passed": False,
                    "exit_code": 1,
                },
            )
        }
    )
    candidate_trace = candidate.traces[0].model_copy(
        update={
            "forge_run_evidence": ForgeRunEvidence(
                source="forge_headless_trace",
                session_id="candidate-session",
                turn_id="candidate-turn",
                verification={
                    "command": "python -c 'pass'",
                    "passed": True,
                    "exit_code": 1,
                },
            )
        }
    )
    control = ProviderABArm(
        arm_id=control.arm_id,
        dataset_fingerprint=control.dataset_fingerprint,
        traces=(control_trace,),
        report=build_report([control_trace]).model_copy(
            update={"trust_result": TrustGateResult(status=TrustStatus.TRUSTED, trusted=True)}
        ),
    )
    candidate = ProviderABArm(
        arm_id=candidate.arm_id,
        dataset_fingerprint=candidate.dataset_fingerprint,
        traces=(candidate_trace,),
        report=build_report([candidate_trace]).model_copy(
            update={"trust_result": TrustGateResult(status=TrustStatus.TRUSTED, trusted=True)}
        ),
    )

    comparison = compare_provider_ab(control=control, candidate=candidate)

    assert (
        "candidate_score_regression:forge_verification_evidence_quality_ok" in comparison.blockers
    )
    assert comparison.winner == "blocked"


def test_provider_ab_handles_conditional_harness_score_missing_on_one_attempt() -> None:
    evidence = ForgeRunEvidence(
        source="forge_headless_trace",
        session_id="session",
        turn_id="turn",
        verification={
            "command": "python -c 'pass'",
            "passed": True,
            "exit_code": 0,
        },
    )

    def attach_first_attempt(arm: ProviderABArm) -> ProviderABArm:
        trace = arm.traces[0].model_copy(update={"forge_run_evidence": evidence})
        traces = (trace, arm.traces[1])
        return ProviderABArm(
            arm_id=arm.arm_id,
            dataset_fingerprint=arm.dataset_fingerprint,
            traces=traces,
            report=build_report(list(traces)).model_copy(
                update={"trust_result": TrustGateResult(status=TrustStatus.TRUSTED, trusted=True)}
            ),
        )

    comparison = compare_provider_ab(
        control=attach_first_attempt(_deepseek(["task-a", "task-b"])),
        candidate=attach_first_attempt(_kimi(["task-a", "task-b"])),
    )

    assert comparison.winner == "no_material_difference"
    assert "score_forge_verification_evidence_quality_ok" not in comparison.bootstrap_intervals


def test_provider_ab_binds_run_contract_to_comparison() -> None:
    contract = ProviderABRunContract(
        trials=1,
        minimum_paired_attempts=2,
        max_total_cost_usd=0.2,
        require_known_cost=True,
        max_total_runtime_seconds=600.0,
        include_red_team=True,
        require_red_team=True,
    )
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"]),
        run_contract=contract,
    )

    assert comparison.run_contract == contract


def test_provider_ab_contract_blocks_single_pair_capability_claim() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"], passed=[False]),
        candidate=_kimi(["task-a"], passed=[True]),
        run_contract=ProviderABRunContract(
            trials=1,
            minimum_paired_attempts=2,
        ),
    )

    assert comparison.winner == "blocked"
    assert "insufficient_paired_attempts:1/2" in comparison.blockers


def test_provider_ab_contract_defaults_to_two_paired_attempts() -> None:
    assert ProviderABRunContract(trials=1).minimum_paired_attempts == 2


def test_provider_ab_distribution_uses_standard_even_sample_median() -> None:
    first = _trace(
        task_id="task-a",
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic_messages",
        endpoint_family="anthropic",
        base_url="https://api.deepseek.com/anthropic",
    )
    second = first.model_copy(update={"model_rounds": 4, "duration_ms": 300, "cost_usd": 0.03})

    summary = _distribution_summary((first, second))

    assert summary["median_model_rounds"] == 3.0
    assert summary["median_duration_ms"] == 200.0
    assert summary["median_cost_usd"] == 0.02
    assert summary["p95_model_rounds"] == 4.0


def test_provider_ab_contract_reports_failure_category_delta() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a", "task-b"], passed=[True, False]),
        candidate=_kimi(["task-a", "task-b"], passed=[True, True]),
        run_contract=ProviderABRunContract(
            trials=1,
            minimum_paired_attempts=2,
        ),
    )

    assert comparison.control.failure_categories == {"runner_error": 1}
    assert comparison.candidate.failure_categories == {}
    assert comparison.failure_category_deltas == {"runner_error": -1}


def test_provider_ab_contract_reports_bounded_provider_error_delta() -> None:
    control = _deepseek(["task-a"], passed=[False])
    control_trace = control.traces[0].model_copy(
        update={
            "error": "HTTP 429: provider token=should-not-appear",
            "raw_events": [
                *control.traces[0].raw_events,
                {
                    "event_type": "provider_error",
                    "error": "HTTP 429: provider token=should-not-appear",
                },
            ],
        }
    )
    control = ProviderABArm(
        arm_id=control.arm_id,
        dataset_fingerprint=control.dataset_fingerprint,
        traces=(control_trace,),
        report=control.report,
    )

    comparison = compare_provider_ab(
        control=control,
        candidate=_kimi(["task-a"]),
        run_contract=ProviderABRunContract(trials=1, minimum_paired_attempts=2),
    )

    assert comparison.control.provider_errors == {"rate_limit": 1}
    assert comparison.candidate.provider_errors == {}
    assert comparison.provider_error_deltas == {"rate_limit": -1}
    assert all("token" not in key for key in comparison.control.provider_errors)


def test_provider_ab_prefers_controller_failure_taxonomy_over_legacy_report() -> None:
    control = _deepseek(["task-a"], passed=[False])
    trace = control.traces[0].model_copy(
        update={
            "failure_category": FailureCategory.SCOPE_VIOLATION,
            "scope_violations": ["unexpected_change:generated.txt"],
            "raw_events": [
                *control.traces[0].raw_events,
                {
                    "event_type": "harness_contract_evidence",
                    "failure_taxonomy": {"primary": "scope_violation"},
                },
            ],
        }
    )
    control = ProviderABArm(
        arm_id=control.arm_id,
        dataset_fingerprint=control.dataset_fingerprint,
        traces=(trace,),
        report=control.report.model_copy(update={"failure_categories": {"runner_error": 1}}),
    )

    comparison = compare_provider_ab(
        control=control,
        candidate=_kimi(["task-a"]),
        run_contract=ProviderABRunContract(trials=1, minimum_paired_attempts=2),
    )

    assert comparison.control.failure_categories == {"scope_violation": 1}


def test_provider_ab_exposes_universal_score_intervals() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"]),
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )

    for name in (
        "functional_correctness",
        "scope_ok",
        "regression_ok",
        "bugfix_ok",
        "secret_leak_ok",
        "prompt_injection_ok",
        "scope_escape_ok",
        "future_state_leakage_ok",
        "unsafe_tool_use_ok",
    ):
        assert f"score_{name}" in comparison.bootstrap_intervals


def test_provider_ab_does_not_call_candidate_winner_with_safety_regression(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from app.scoring import UNIVERSAL_SCORE_NAMES

    def fake_score_trace(trace: AgentTrace) -> dict[str, EvalScore]:
        return {
            name: EvalScore(
                name=name,
                score=0.0 if name == "secret_leak_ok" and trace.model == "k3" else 1.0,
                label="failed" if name == "secret_leak_ok" and trace.model == "k3" else "ok",
            )
            for name in UNIVERSAL_SCORE_NAMES
        }

    monkeypatch.setattr("app.provider_ab.score_trace", fake_score_trace)
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"]),
    )

    assert comparison.bootstrap_intervals["score_secret_leak_ok"].high < 0
    assert comparison.winner == "control"


def test_provider_ab_rejects_task_order_or_dataset_mismatch() -> None:
    with pytest.raises(ProviderABError, match="task_order_mismatch"):
        compare_provider_ab(
            control=_deepseek(["task-a", "task-b"]),
            candidate=_kimi(["task-b", "task-a"]),
        )

    with pytest.raises(ProviderABError, match="dataset_fingerprint_mismatch"):
        compare_provider_ab(
            control=_deepseek(["task-a", "task-b"]),
            candidate=_kimi(["task-a", "task-b"], dataset_fingerprint="b" * 64),
        )


def test_provider_ab_blocks_untrusted_arm_instead_of_declaring_winner() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"], trusted=False),
    )

    assert comparison.winner == "blocked"
    assert comparison.blockers == ["candidate:workspace_untrusted"]


def test_provider_ab_blocks_unknown_provider_cost() -> None:
    candidate = _kimi(["task-a"])
    candidate = ProviderABArm(
        arm_id=candidate.arm_id,
        dataset_fingerprint=candidate.dataset_fingerprint,
        traces=candidate.traces,
        report=candidate.report.model_copy(update={"provider_cost_known_rate": 0.0}),
    )

    comparison = compare_provider_ab(control=_deepseek(["task-a"]), candidate=candidate)

    assert comparison.winner == "blocked"
    assert comparison.blockers == ["candidate:provider_cost_unknown"]


def test_provider_ab_rejects_fallback_contamination() -> None:
    with pytest.raises(ProviderABError, match="fallback_contamination"):
        compare_provider_ab(
            control=_deepseek(["task-a"]),
            candidate=_kimi(["task-a"], fallback=True),
        )


def test_run_provider_ab_executes_two_route_pure_arms(
    tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls: list[object] = []
    cancel_callbacks: list[object] = []

    monkeypatch.setattr(
        "app.provider_ab.load_cases",
        lambda _path: [EvaluationTask(id="task-a", title="A", prompt="Do A.")],
    )

    def fake_execute(*, options, cancel_requested, **_kwargs):
        calls.append(options)
        cancel_callbacks.append(cancel_requested)
        arm = (
            _deepseek(["task-a"])
            if options.provider_route.provider == "deepseek"
            else _kimi(["task-a"])
        )
        return EvaluationExecution(
            traces=list(arm.traces),
            report=arm.report,
            trust_result=arm.report.trust_result,
        )

    monkeypatch.setattr("app.provider_ab.execute_evaluation", fake_execute)

    result = run_provider_ab(
        cases_path=tmp_path / "cases.json",
        forge_command=["forge"],
        max_total_cost_usd=0.02,
    )

    assert [call.provider_route.provider for call in calls] == ["deepseek", "custom_openai"]
    assert [call.max_total_cost_usd for call in calls] == [0.02, 0.01]
    assert all(call.provider_chain is None for call in calls)
    assert result.comparison.paired_attempts == 1
    assert len(result.comparison.source_commit) == 40
    assert len(result.comparison.dirty_state_sha256) == 64
    assert result.comparison.task_ids == ["task-a"]
    assert result.comparison.run_contract is not None
    assert result.comparison.run_contract.minimum_paired_attempts == 2
    assert result.comparison.control.distribution_summary == {
        "median_model_rounds": 2.0,
        "median_duration_ms": 100.0,
        "median_cost_usd": 0.01,
        "p95_model_rounds": 2.0,
        "p95_duration_ms": 100.0,
        "p95_cost_usd": 0.01,
    }
    assert result.comparison.control.failure_categories == result.control.report.failure_categories
    assert result.comparison.failure_category_deltas == {}
    assert result.comparison.control.harness_telemetry is not None
    assert result.comparison.control.harness_telemetry.attempts_with_contract == 0
    assert result.comparison.control.harness_telemetry.contract_coverage == 0.0
    assert len(cancel_callbacks) == 2
    assert all(callable(callback) for callback in cancel_callbacks)


def test_run_provider_ab_rejects_non_positive_runtime_budget(tmp_path) -> None:
    with pytest.raises(ProviderABError, match="max_total_runtime_seconds_must_be_positive"):
        run_provider_ab(
            cases_path=tmp_path / "cases.json",
            forge_command=["forge"],
            max_total_runtime_seconds=0,
        )


def test_run_provider_ab_stops_before_second_arm_when_pair_budget_is_spent(
    tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        "app.provider_ab.load_cases",
        lambda _path: [EvaluationTask(id="task-a", title="A", prompt="Do A.")],
    )

    def fake_execute(*, options, **_kwargs):
        calls.append(options.provider_route.provider)
        arm = (
            _deepseek(["task-a"])
            if options.provider_route.provider == "deepseek"
            else _kimi(["task-a"])
        )
        return EvaluationExecution(
            traces=list(arm.traces),
            report=arm.report,
            trust_result=arm.report.trust_result,
        )

    monkeypatch.setattr("app.provider_ab.execute_evaluation", fake_execute)

    with pytest.raises(ProviderABError, match="paired_cost_budget_exhausted:kimi_k3"):
        run_provider_ab(
            cases_path=tmp_path / "cases.json",
            forge_command=["forge"],
            max_total_cost_usd=0.01,
        )

    assert calls == ["deepseek"]


def test_provider_ab_cli_rejects_non_positive_runtime_budget() -> None:
    with pytest.raises(SystemExit) as exc_info:
        provider_ab_main(
            [
                "--cases",
                "cases",
                "--output",
                "artifact.json",
                "--max-total-runtime-seconds",
                "0",
            ]
        )
    assert exc_info.value.code == 2


def test_run_provider_ab_reports_incomplete_arm_without_pairing(
    tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        "app.provider_ab.load_cases",
        lambda _path: [EvaluationTask(id="task-a", title="A", prompt="Do A.")],
    )
    empty_report = build_report([])

    def fake_execute(**_kwargs):
        return EvaluationExecution(
            traces=[],
            report=empty_report,
            trust_result=empty_report.trust_result,
        )

    monkeypatch.setattr("app.provider_ab.execute_evaluation", fake_execute)

    with pytest.raises(ProviderABError, match="arm_incomplete:deepseek_v4_flash:0/1"):
        run_provider_ab(cases_path=tmp_path / "cases.json", forge_command=["forge"])


def test_provider_ab_artifact_has_canonical_sha256_sidecar(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )

    output = tmp_path / "provider-ab.json"
    digest = write_provider_ab_artifact(
        output,
        comparison=comparison,
        control=control,
        candidate=candidate,
    )

    assert output.exists()
    assert output.with_suffix(".json.sha256").read_text().strip() == digest
    assert "api_key" not in output.read_text().lower()


def test_provider_ab_artifact_rejects_missing_source_provenance(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(control=control, candidate=candidate)

    with pytest.raises(ProviderABError, match="source_provenance_missing"):
        write_provider_ab_artifact(
            tmp_path / "provider-ab.json",
            comparison=comparison,
            control=control,
            candidate=candidate,
        )


def test_provider_ab_artifact_rejects_tampered_comparison_summary(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )
    tampered = comparison.model_copy(update={"winner": "candidate"})

    with pytest.raises(ProviderABError, match="comparison_mismatch"):
        write_provider_ab_artifact(
            tmp_path / "provider-ab.json",
            comparison=tampered,
            control=control,
            candidate=candidate,
        )


def test_provider_ab_artifact_rejects_secret_like_trace_content(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )
    leaked_trace = candidate.traces[0].model_copy(
        update={
            "raw_events": [
                *candidate.traces[0].raw_events,
                {"event_type": "debug", "message": "sk-1234567890abcdef"},
            ]
        }
    )
    leaked_candidate = ProviderABArm(
        arm_id=candidate.arm_id,
        dataset_fingerprint=candidate.dataset_fingerprint,
        traces=(leaked_trace,),
        report=candidate.report,
    )

    with pytest.raises(ProviderABError, match="artifact_secret_detected"):
        write_provider_ab_artifact(
            tmp_path / "provider-ab.json",
            comparison=comparison,
            control=control,
            candidate=leaked_candidate,
        )


def test_provider_ab_artifact_verifier_round_trips_and_rejects_tamper(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )
    output = tmp_path / "provider-ab.json"
    write_provider_ab_artifact(
        output,
        comparison=comparison,
        control=control,
        candidate=candidate,
    )

    verified = load_provider_ab_artifact(output)

    assert verified.comparison == comparison
    assert verified.control.traces == control.traces
    output.write_bytes(output.read_bytes().replace(b'"task-a"', b'"task-b"', 1))
    with pytest.raises(ProviderABError, match="artifact_digest_mismatch"):
        load_provider_ab_artifact(output)

    write_provider_ab_artifact(
        output,
        comparison=comparison,
        control=control,
        candidate=candidate,
    )
    noncanonical = json.dumps(json.loads(output.read_text()), indent=2).encode() + b"\n"
    output.write_bytes(noncanonical)
    output.with_suffix(".json.sha256").write_text(sha256_bytes(noncanonical) + "\n")
    with pytest.raises(ProviderABError, match="artifact_not_canonical"):
        load_provider_ab_artifact(output)


def test_provider_ab_verifier_preserves_legacy_contract_without_minimum(tmp_path) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    legacy_contract = ProviderABRunContract(trials=1, minimum_paired_attempts=None)
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        run_contract=legacy_contract,
        task_ids=["task-a"],
    )
    output = tmp_path / "legacy-provider-ab.json"
    write_provider_ab_artifact(
        output,
        comparison=comparison,
        control=control,
        candidate=candidate,
    )
    payload = json.loads(output.read_text())
    del payload["comparison"]["run_contract"]["minimum_paired_attempts"]
    raw = canonical_json_bytes(payload) + b"\n"
    output.write_bytes(raw)
    output.with_suffix(".json.sha256").write_text(sha256_bytes(raw) + "\n")

    verified = load_provider_ab_artifact(output)

    assert verified.comparison.run_contract is not None
    assert verified.comparison.run_contract.minimum_paired_attempts is None


def test_provider_ab_verify_cli_is_offline_and_secret_free(tmp_path, capsys) -> None:
    control = _deepseek(["task-a"])
    candidate = _kimi(["task-a"])
    comparison = compare_provider_ab(
        control=control,
        candidate=candidate,
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )
    output = tmp_path / "provider-ab.json"
    write_provider_ab_artifact(
        output,
        comparison=comparison,
        control=control,
        candidate=candidate,
    )

    assert provider_ab_verify_main(["--artifact", str(output)]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["ok"] is True
    assert payload["artifact_sha256"] == sha256_bytes(output.read_bytes())
    assert "traces" not in payload
    assert "api_key" not in json.dumps(payload).lower()


def test_provider_ab_binds_source_and_task_contract_to_comparison() -> None:
    comparison = compare_provider_ab(
        control=_deepseek(["task-a"]),
        candidate=_kimi(["task-a"]),
        source_commit="1" * 40,
        dirty_state_sha256="2" * 64,
        task_ids=["task-a"],
    )

    assert comparison.source_commit == "1" * 40
    assert comparison.dirty_state_sha256 == "2" * 64
    assert comparison.task_ids == ["task-a"]
