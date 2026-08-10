import json
import stat
from decimal import Decimal
from pathlib import Path

import pytest

from app.provenance import (
    append_entry,
    canonical_json_bytes,
    entry_digest,
    sha256_bytes,
    verify_ledger,
)
from app.provenance_models import ForgeProvenanceEntry

GIT_A = "a" * 40
GIT_B = "b" * 40
GIT_C = "c" * 40
SHA_A = "a" * 64
SHA_B = "b" * 64


def write_artifacts(
    root: Path,
    run_id: str,
    *,
    trace: bytes = b'{"trace":"ok"}\n',
    report: bytes = b'{"report":"ok"}\n',
    patch: bytes = b"diff --git a/a.txt b/a.txt\n",
    verification: bytes = b'{"passed":true}\n',
) -> dict[str, str]:
    contents = {
        "traces": (".json", trace),
        "reports": (".json", report),
        "patches": (".patch", patch),
        "verification": (".json", verification),
    }
    digests = {}
    for kind, (suffix, content) in contents.items():
        path = root / kind / f"{run_id}{suffix}"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        digests[kind] = sha256_bytes(content)
    return digests


def make_entry(
    artifacts_root: Path,
    *,
    sequence: int = 1,
    run_id: str = "run-1",
    task_id: str = "task-1",
    previous_entry_sha256: str | None = None,
    input_commit: str = GIT_A,
    output_commit: str = GIT_B,
    trace: bytes = b'{"trace":"ok"}\n',
    patch: bytes | None = None,
    **updates,
) -> ForgeProvenanceEntry:
    if patch is None:
        patch = f"diff --git a/{run_id}.txt b/{run_id}.txt\n".encode()
    digests = write_artifacts(artifacts_root, run_id, trace=trace, patch=patch)
    payload = {
        "sequence": sequence,
        "previous_entry_sha256": previous_entry_sha256,
        "task_id": task_id,
        "run_id": run_id,
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic-compatible",
        "input_commit": input_commit,
        "patch_sha256": digests["patches"],
        "output_commit": output_commit,
        "trace_sha256": digests["traces"],
        "report_sha256": digests["reports"],
        "harness_fingerprint": SHA_A,
        "dataset_fingerprint": SHA_B,
        "verification_digest": digests["verification"],
        "cost_cny": Decimal("0.25"),
    }
    payload.update(updates)
    return ForgeProvenanceEntry.model_validate(payload)


def test_canonical_json_is_order_independent_and_rejects_nan() -> None:
    left = {"b": [2, 1], "a": {"z": False, "x": "中文"}}
    right = {"a": {"x": "中文", "z": False}, "b": [2, 1]}

    assert canonical_json_bytes(left) == canonical_json_bytes(right)
    assert canonical_json_bytes(left) == (
        b'{"a":{"x":"\xe4\xb8\xad\xe6\x96\x87","z":false},"b":[2,1]}'
    )
    with pytest.raises(ValueError):
        canonical_json_bytes({"bad": float("nan")})


def test_append_writes_one_canonical_fsynced_private_line(tmp_path: Path) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    entry = make_entry(artifacts)

    appended_digest = append_entry(ledger, entry, artifacts)
    verification = verify_ledger(ledger, artifacts)

    assert appended_digest == entry_digest(entry)
    assert ledger.read_bytes() == canonical_json_bytes(entry.model_dump(mode="json")) + b"\n"
    assert stat.S_IMODE(ledger.stat().st_mode) == 0o600
    assert verification.ok is True
    assert verification.entry_count == 1
    assert verification.final_entry_sha256 == appended_digest
    assert verification.task_ids == ["task-1"]
    assert verification.run_ids == ["run-1"]
    assert verification.output_commits == [GIT_B]


def test_append_requires_exact_chain_and_unique_identity(tmp_path: Path) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    first = make_entry(artifacts)
    first_digest = append_entry(ledger, first, artifacts)

    second = make_entry(
        artifacts,
        sequence=2,
        run_id="run-2",
        task_id="task-2",
        previous_entry_sha256=first_digest,
        input_commit=GIT_B,
        output_commit=GIT_C,
    )
    append_entry(ledger, second, artifacts)
    assert verify_ledger(ledger, artifacts).ok is True

    invalid_updates = (
        {"sequence": 4, "run_id": "run-3", "task_id": "task-3"},
        {"run_id": "run-1", "task_id": "task-3"},
        {"run_id": "run-3", "task_id": "task-1"},
        {"run_id": "run-3", "task_id": "task-3", "patch_sha256": first.patch_sha256},
        {"run_id": "run-3", "task_id": "task-3", "output_commit": GIT_B},
        {"run_id": "run-3", "task_id": "task-3", "input_commit": GIT_A},
        {"run_id": "run-3", "task_id": "task-3", "previous_entry_sha256": SHA_A},
    )
    for index, updates in enumerate(invalid_updates):
        candidate_args = {
            "sequence": 3,
            "run_id": f"candidate-{index}",
            "task_id": f"candidate-{index}",
            "previous_entry_sha256": entry_digest(second),
            "input_commit": GIT_C,
            "output_commit": str(index) * 40,
            **updates,
        }
        candidate = make_entry(artifacts, **candidate_args)
        before = ledger.read_bytes()
        with pytest.raises(ValueError):
            append_entry(ledger, candidate, artifacts)
        assert ledger.read_bytes() == before


@pytest.mark.parametrize(
    "mutation",
    ["truncate", "reorder", "replace", "malformed", "blank", "extra"],
)
def test_ledger_tampering_is_detected(tmp_path: Path, mutation: str) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    first = make_entry(artifacts)
    first_digest = append_entry(ledger, first, artifacts)
    second = make_entry(
        artifacts,
        sequence=2,
        run_id="run-2",
        task_id="task-2",
        previous_entry_sha256=first_digest,
        input_commit=GIT_B,
        output_commit=GIT_C,
    )
    append_entry(ledger, second, artifacts)
    lines = ledger.read_bytes().splitlines(keepends=True)

    if mutation == "truncate":
        ledger.write_bytes(b"".join(lines)[:-1])
    elif mutation == "reorder":
        ledger.write_bytes(lines[1] + lines[0])
    elif mutation == "replace":
        payload = json.loads(lines[0])
        payload["task_id"] = "changed"
        ledger.write_bytes(canonical_json_bytes(payload) + b"\n" + lines[1])
    elif mutation == "malformed":
        ledger.write_bytes(lines[0] + b"{bad json}\n")
    elif mutation == "blank":
        ledger.write_bytes(lines[0] + b"\n" + lines[1])
    else:
        ledger.write_bytes(b"".join(lines) + b"not-json\n")

    result = verify_ledger(ledger, artifacts)

    assert result.ok is False
    assert result.blockers


@pytest.mark.parametrize(
    ("kind", "suffix"),
    [
        ("traces", ".json"),
        ("reports", ".json"),
        ("patches", ".patch"),
        ("verification", ".json"),
    ],
)
def test_ledger_recomputes_every_artifact_hash(
    tmp_path: Path,
    kind: str,
    suffix: str,
) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    entry = make_entry(artifacts)
    append_entry(ledger, entry, artifacts)

    (artifacts / kind / f"run-1{suffix}").write_bytes(b"tampered")

    result = verify_ledger(ledger, artifacts)

    assert result.ok is False
    assert f"artifact_digest_mismatch:run-1:{kind}" in result.blockers


def test_ledger_rejects_secret_bearing_artifacts_even_when_hash_matches(
    tmp_path: Path,
) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    secret_trace = b'{"authorization":"Bearer abcdefghijklmnopqrstuvwxyz"}\n'
    entry = make_entry(
        artifacts,
        run_id="run-secret",
        trace=secret_trace,
    )

    with pytest.raises(ValueError, match="secret"):
        append_entry(ledger, entry, artifacts)
    assert not ledger.exists()


def test_first_entry_cannot_claim_a_previous_hash(tmp_path: Path) -> None:
    ledger = tmp_path / "ledger.jsonl"
    artifacts = tmp_path / "artifacts"
    entry = make_entry(artifacts, previous_entry_sha256=SHA_A)

    with pytest.raises(ValueError, match="previous"):
        append_entry(ledger, entry, artifacts)
    assert not ledger.exists()
