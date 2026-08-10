import os
import re
import sys
from pathlib import Path

from app.artifact_hashing import canonical_manifest_digest, canonical_tree_manifest
from app.models import ShellOutput
from app.process_control import CancelRequested, never_cancelled, run_bounded_process

VERIFIER_ROOT = Path(__file__).resolve().parents[1] / "forgebench" / "verifiers"
VERIFIER_ID_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]*")
MAX_VERIFIER_OUTPUT_CHARS = 12_000


def verifier_command(verifier_id: str, workspace: Path) -> tuple[list[str], str]:
    if VERIFIER_ID_PATTERN.fullmatch(verifier_id) is None:
        raise ValueError("invalid hidden verifier id")
    verifier_path = VERIFIER_ROOT / f"{verifier_id}.py"
    if not verifier_path.is_file():
        raise ValueError(f"unknown hidden verifier: {verifier_id}")
    if not workspace.is_dir():
        raise ValueError(f"hidden verifier workspace does not exist: {workspace}")

    required_paths = {"_support.py", verifier_path.name}
    entries = [
        entry for entry in canonical_tree_manifest(VERIFIER_ROOT) if entry["path"] in required_paths
    ]
    if {str(entry["path"]) for entry in entries} != required_paths:
        raise ValueError(f"hidden verifier support is incomplete: {verifier_id}")
    digest = canonical_manifest_digest(entries)
    return [sys.executable, str(verifier_path), str(workspace.resolve())], digest


def run_hidden_verifier(
    verifier_id: str,
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
) -> ShellOutput:
    command, digest = verifier_command(verifier_id, workspace)
    environment = os.environ.copy()
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    completed = run_bounded_process(
        command,
        cwd=workspace,
        timeout_seconds=timeout_seconds,
        cancel_requested=cancel_requested,
        env=environment,
    )
    return ShellOutput(
        command=f"hidden-verifier:{verifier_id}:sha256:{digest}",
        stdout=_bounded_output(completed.stdout),
        stderr=_bounded_output(completed.stderr),
        exit_code=completed.returncode,
        duration_ms=completed.duration_ms,
    )


def _bounded_output(value: str) -> str:
    return value[:MAX_VERIFIER_OUTPUT_CHARS]
