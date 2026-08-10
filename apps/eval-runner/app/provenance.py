import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Any

from pydantic import ValidationError

from app.models import EvalModel
from app.provenance_models import ForgeProvenanceEntry

SECRET_BYTES_PATTERN = rb"(?i)(?:sk-[a-z0-9_-]{16,}|api[_-]?key\s*[:=]|bearer\s+[a-z0-9._-]{12,})"

ARTIFACT_SPECS = {
    "traces": (".json", "trace_sha256"),
    "reports": (".json", "report_sha256"),
    "patches": (".patch", "patch_sha256"),
    "verification": (".json", "verification_digest"),
}


class LedgerVerification(EvalModel):
    ok: bool
    entry_count: int
    final_entry_sha256: str | None = None
    blockers: list[str] = []
    task_ids: list[str] = []
    run_ids: list[str] = []
    output_commits: list[str] = []


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def entry_digest(entry: ForgeProvenanceEntry) -> str:
    return sha256_bytes(canonical_json_bytes(entry.model_dump(mode="json")))


def append_entry(
    ledger_path: Path,
    entry: ForgeProvenanceEntry,
    artifacts_root: Path,
) -> str:
    existing_result, entries = _read_and_verify_ledger(ledger_path, artifacts_root)
    if not existing_result.ok:
        raise ValueError(
            "existing provenance ledger is invalid: " + ", ".join(existing_result.blockers)
        )

    candidate_blockers = _verify_entries([*entries, entry], artifacts_root)
    if candidate_blockers:
        raise ValueError("provenance entry is invalid: " + ", ".join(candidate_blockers))

    ledger_path.parent.mkdir(parents=True, exist_ok=True)
    line = canonical_json_bytes(entry.model_dump(mode="json")) + b"\n"
    descriptor = os.open(
        ledger_path,
        os.O_APPEND | os.O_CREAT | os.O_WRONLY,
        0o600,
    )
    try:
        if os.write(descriptor, line) != len(line):
            raise OSError("short append while writing provenance ledger")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.chmod(ledger_path, 0o600)
    return entry_digest(entry)


def verify_ledger(ledger_path: Path, artifacts_root: Path) -> LedgerVerification:
    result, _entries = _read_and_verify_ledger(ledger_path, artifacts_root)
    return result


def load_verified_entries(
    ledger_path: Path,
    artifacts_root: Path,
) -> list[ForgeProvenanceEntry]:
    result, entries = _read_and_verify_ledger(ledger_path, artifacts_root)
    if not result.ok:
        raise ValueError("provenance ledger is invalid: " + ", ".join(result.blockers))
    return entries


def _read_and_verify_ledger(
    ledger_path: Path,
    artifacts_root: Path,
) -> tuple[LedgerVerification, list[ForgeProvenanceEntry]]:
    if not ledger_path.exists():
        return _result([], []), []

    blockers: list[str] = []
    mode = stat.S_IMODE(ledger_path.stat().st_mode)
    if mode != 0o600:
        blockers.append("ledger_mode_not_0600")

    content = ledger_path.read_bytes()
    if not content:
        return _result([], blockers), []
    if not content.endswith(b"\n"):
        blockers.append("ledger_not_newline_terminated")

    raw_lines = content.split(b"\n")
    if raw_lines[-1] == b"":
        raw_lines.pop()

    entries: list[ForgeProvenanceEntry] = []
    for index, raw_line in enumerate(raw_lines, start=1):
        if not raw_line:
            blockers.append(f"blank_ledger_line:{index}")
            continue
        try:
            payload = json.loads(raw_line)
            if not isinstance(payload, dict):
                raise ValueError("ledger entry must be an object")
            entry = ForgeProvenanceEntry.model_validate(payload)
        except (json.JSONDecodeError, ValidationError, ValueError):
            blockers.append(f"malformed_ledger_line:{index}")
            continue
        if raw_line != canonical_json_bytes(entry.model_dump(mode="json")):
            blockers.append(f"non_canonical_ledger_line:{index}")
        entries.append(entry)

    blockers.extend(_verify_entries(entries, artifacts_root))
    return _result(entries, blockers), entries


def _result(
    entries: list[ForgeProvenanceEntry],
    blockers: list[str],
) -> LedgerVerification:
    return LedgerVerification(
        ok=not blockers,
        entry_count=len(entries),
        final_entry_sha256=entry_digest(entries[-1]) if entries else None,
        blockers=list(dict.fromkeys(blockers)),
        task_ids=[entry.task_id for entry in entries],
        run_ids=[entry.run_id for entry in entries],
        output_commits=[entry.output_commit for entry in entries],
    )


def _verify_entries(
    entries: list[ForgeProvenanceEntry],
    artifacts_root: Path,
) -> list[str]:
    blockers: list[str] = []
    seen_task_ids: set[str] = set()
    seen_run_ids: set[str] = set()
    seen_patch_hashes: set[str] = set()
    seen_output_commits: set[str] = set()
    previous: ForgeProvenanceEntry | None = None

    for expected_sequence, entry in enumerate(entries, start=1):
        if entry.sequence != expected_sequence:
            blockers.append(f"sequence_mismatch:{entry.run_id}")
        expected_previous = entry_digest(previous) if previous is not None else None
        if entry.previous_entry_sha256 != expected_previous:
            blockers.append(f"previous_entry_mismatch:{entry.run_id}")
        if previous is not None and entry.input_commit != previous.output_commit:
            blockers.append(f"commit_continuity_mismatch:{entry.run_id}")

        _check_unique(blockers, seen_task_ids, entry.task_id, "duplicate_task_id")
        _check_unique(blockers, seen_run_ids, entry.run_id, "duplicate_run_id")
        _check_unique(
            blockers,
            seen_patch_hashes,
            entry.patch_sha256,
            "duplicate_patch_sha256",
        )
        _check_unique(
            blockers,
            seen_output_commits,
            entry.output_commit,
            "duplicate_output_commit",
        )
        blockers.extend(_verify_entry_artifacts(entry, artifacts_root))
        previous = entry
    return blockers


def _check_unique(
    blockers: list[str],
    seen: set[str],
    value: str,
    code: str,
) -> None:
    if value in seen:
        blockers.append(f"{code}:{value}")
    seen.add(value)


def _verify_entry_artifacts(
    entry: ForgeProvenanceEntry,
    artifacts_root: Path,
) -> list[str]:
    import re

    blockers: list[str] = []
    secret_pattern = re.compile(SECRET_BYTES_PATTERN)
    for kind, (suffix, digest_field) in ARTIFACT_SPECS.items():
        path = artifacts_root / kind / f"{entry.run_id}{suffix}"
        if not path.is_file():
            blockers.append(f"artifact_missing:{entry.run_id}:{kind}")
            continue
        content = path.read_bytes()
        if secret_pattern.search(content) is not None:
            blockers.append(f"artifact_secret_detected:{entry.run_id}:{kind}")
        expected_digest = str(getattr(entry, digest_field))
        if sha256_bytes(content) != expected_digest:
            blockers.append(f"artifact_digest_mismatch:{entry.run_id}:{kind}")
    return blockers
