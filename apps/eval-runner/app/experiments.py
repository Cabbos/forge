from app.datasets import dataset_fingerprint
from app.models import EvaluationTask


def experiment_artifact_metadata(
    *,
    name: str,
    tasks: list[EvaluationTask],
    provider: str,
    model: str,
) -> dict[str, str]:
    return {
        "name": name,
        "dataset_fingerprint": dataset_fingerprint(tasks),
        "provider": provider,
        "model": model,
    }
