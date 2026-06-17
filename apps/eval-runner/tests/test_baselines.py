import json


def minimal_artifact(success_rate: float = 1.0) -> dict:
    return {
        "report": {
            "total_tasks": 1,
            "success_rate": success_rate,
            "verification_pass_rate": success_rate,
            "scope_violation_rate": 0.0,
            "avg_duration_ms": 10.0,
            "avg_model_rounds": 1.0,
            "avg_confirm_requests": 0.0,
            "failure_categories": {},
            "score_summary": {"functional_correctness": success_rate},
            "tasks": [],
        },
        "traces": [],
        "experiment": {
            "name": "local-regression",
            "dataset_fingerprint": "dataset-123",
            "provider": "mock",
            "model": "deterministic-agent-v1",
        },
    }


def test_baseline_registry_promotes_latest_trusted_report(tmp_path):
    from app.baselines import BaselineRegistry

    artifact = tmp_path / "run.json"
    artifact.write_text(json.dumps(minimal_artifact()), encoding="utf-8")
    registry = BaselineRegistry(tmp_path / "baselines.json")

    record = registry.promote(
        artifact_path=artifact,
        name="local-regression",
        trusted=True,
        note="green release gate",
    )

    latest = registry.latest(name="local-regression")
    assert latest is not None
    assert latest.artifact_path == str(artifact)
    assert latest.dataset_fingerprint == "dataset-123"
    assert latest.success_rate == 1.0
    assert latest.trusted is True
    assert latest.note == "green release gate"
    assert record == latest


def test_baseline_registry_ignores_untrusted_for_latest_trusted(tmp_path):
    from app.baselines import BaselineRegistry

    good = tmp_path / "good.json"
    bad = tmp_path / "bad.json"
    good.write_text(json.dumps(minimal_artifact(1.0)), encoding="utf-8")
    bad.write_text(json.dumps(minimal_artifact(0.0)), encoding="utf-8")
    registry = BaselineRegistry(tmp_path / "baselines.json")

    registry.promote(artifact_path=good, name="local-regression", trusted=True)
    registry.promote(artifact_path=bad, name="local-regression", trusted=False)

    latest = registry.latest(name="local-regression")
    assert latest is not None
    assert latest.artifact_path == str(good)
