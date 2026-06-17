from app.metrics import trace_passed
from app.models import AgentTrace, EvalScore


def score_trace(
    trace: AgentTrace,
    *,
    max_model_rounds: int | None = None,
) -> dict[str, EvalScore]:
    passed = trace_passed(trace)
    scores = {
        "functional_correctness": EvalScore(
            name="functional_correctness",
            score=1.0 if passed else 0.0,
            label="passed" if passed else "failed",
            explanation=trace.failure_reason,
        ),
        "scope_ok": EvalScore(
            name="scope_ok",
            score=0.0 if trace.scope_violations else 1.0,
            label="scope_violation" if trace.scope_violations else "ok",
        ),
    }
    if max_model_rounds is not None:
        over_budget = trace.model_rounds > max_model_rounds
        scores["budget_ok"] = EvalScore(
            name="budget_ok",
            score=0.0 if over_budget else 1.0,
            label="max_model_rounds_exceeded" if over_budget else "ok",
        )
    return scores
