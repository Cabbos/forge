from app.models import EvalScore

JUDGE_SCORE_SOURCES = {"llm_judge", "semantic"}


def scorer_agreement(golden: list[EvalScore], candidate: list[EvalScore]) -> float:
    if not golden:
        return 0.0
    by_name = {score.name: score for score in candidate}
    matches = 0
    for expected in golden:
        actual = by_name.get(expected.name)
        if actual and actual.label == expected.label:
            matches += 1
    return matches / len(golden)


def score_can_gate_ci(score: EvalScore) -> bool:
    if score.source not in JUDGE_SCORE_SOURCES:
        return score.gate_ci
    if not score.gate_ci:
        return False
    if (
        score.calibration_dataset_id is None
        or score.calibration_agreement is None
        or score.calibration_threshold is None
    ):
        return False
    return score.calibration_agreement >= score.calibration_threshold
