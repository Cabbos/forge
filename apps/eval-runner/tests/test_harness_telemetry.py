from datetime import UTC, datetime

from app.harness_telemetry import summarize_harness_telemetry
from app.models import AgentTrace, EvalProvider, VerificationResult


def trace_with_contract(*, tool_names: list[str], duplicate: bool = False) -> AgentTrace:
    now = datetime.now(UTC)
    names = tool_names + (tool_names[:1] if duplicate and tool_names else [])
    return AgentTrace(
        task_id="telemetry-task",
        user_prompt="measure evidence",
        model="deepseek-v4-flash",
        provider=EvalProvider.FORGE,
        final_answer="done",
        verification_result=VerificationResult(
            command="pytest",
            passed=True,
            exit_code=0,
        ),
        repair_attempts_used=1,
        raw_events=[
            {
                "event_type": "harness_contract_evidence",
                "context_manifest": {
                    "total_bytes": 120,
                    "total_token_estimate": 30,
                    "items": [
                        {
                            "path": "src/a.py",
                            "tier": "task_local",
                            "bytes": 120,
                        }
                    ],
                    "duplicate_groups": [],
                    "conflict_groups": [],
                },
                "tool_results": {
                    "envelopes": [{"tool": name} for name in names],
                },
                "progress": {
                    "stagnation": {"action": "redirect"},
                },
            }
        ],
        started_at=now,
        ended_at=now,
        duration_ms=1,
    )


def test_harness_telemetry_aggregates_tools_context_and_recovery() -> None:
    summary = summarize_harness_telemetry(
        [
            trace_with_contract(tool_names=["read", "edit"], duplicate=True),
            trace_with_contract(tool_names=["read"]),
            AgentTrace(
                task_id="missing-contract",
                user_prompt="measure missing evidence",
                model="deepseek-v4-flash",
                provider=EvalProvider.FORGE,
                final_answer="done",
                started_at=datetime.now(UTC),
                ended_at=datetime.now(UTC),
                duration_ms=0,
            ),
        ]
    )

    assert summary.attempts_with_contract == 2
    assert summary.contract_coverage == 2 / 3
    assert summary.tool_call_histogram == {"1": 1, "3": 1}
    assert summary.repeated_tool_call_rate == 1 / 4
    assert summary.median_context_bytes == 120
    assert summary.context_bytes_by_tier == {"task_local": 240}
    assert summary.stagnation_redirects == 2
    assert summary.fresh_verifier_rate == 0
    assert summary.single_repair_compliance_rate == 1.0


def test_harness_telemetry_empty_input_is_explicitly_zero() -> None:
    summary = summarize_harness_telemetry([])

    assert summary.attempts_with_contract == 0
    assert summary.contract_coverage == 0
    assert summary.tool_call_histogram == {}
