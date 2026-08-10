import json
import sys
from pathlib import Path

import pytest

from app.hidden_verifiers import (
    run_hidden_verifier,
    verifier_command,
)


def write_verifier_root(root: Path, body: str) -> None:
    root.mkdir()
    (root / "_support.py").write_text("OUTPUT_LIMIT = 12000\n", encoding="utf-8")
    (root / "opaque-check.py").write_text(body, encoding="utf-8")


@pytest.mark.parametrize("verifier_id", ["", " ", "../check", "nested/check", "Check"])
def test_verifier_command_rejects_invalid_ids(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    verifier_id: str,
) -> None:
    verifier_root = tmp_path / "verifiers"
    write_verifier_root(verifier_root, "raise SystemExit(0)\n")
    monkeypatch.setattr("app.hidden_verifiers.VERIFIER_ROOT", verifier_root)

    with pytest.raises(ValueError, match="invalid hidden verifier id"):
        verifier_command(verifier_id, tmp_path)


def test_verifier_command_rejects_unknown_id(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    verifier_root = tmp_path / "verifiers"
    write_verifier_root(verifier_root, "raise SystemExit(0)\n")
    monkeypatch.setattr("app.hidden_verifiers.VERIFIER_ROOT", verifier_root)

    with pytest.raises(ValueError, match="unknown hidden verifier"):
        verifier_command("not-there", tmp_path)


def test_hidden_verifier_receives_workspace_and_records_bounded_evidence(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    verifier_root = tmp_path / "verifiers"
    write_verifier_root(
        verifier_root,
        (
            "import json, pathlib, sys\n"
            "workspace = pathlib.Path(sys.argv[1])\n"
            "assert (workspace / 'expected.txt').read_text() == 'ok\\n'\n"
            "print('x' * 20000)\n"
            "print(json.dumps({'ok': True}))\n"
        ),
    )
    monkeypatch.setattr("app.hidden_verifiers.VERIFIER_ROOT", verifier_root)
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "expected.txt").write_text("ok\n", encoding="utf-8")

    command, digest = verifier_command("opaque-check", workspace)
    output = run_hidden_verifier(
        "opaque-check",
        workspace,
        timeout_seconds=5,
    )

    assert command[-1] == str(workspace)
    assert command[0] == sys.executable
    assert len(digest) == 64
    assert output.command == f"hidden-verifier:opaque-check:sha256:{digest}"
    assert output.exit_code == 0
    assert len(output.stdout) == 12000
    assert output.stderr == ""


def test_hidden_verifier_preserves_timeout_and_cancellation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    verifier_root = tmp_path / "verifiers"
    write_verifier_root(verifier_root, "import time\ntime.sleep(30)\n")
    monkeypatch.setattr("app.hidden_verifiers.VERIFIER_ROOT", verifier_root)
    workspace = tmp_path / "workspace"
    workspace.mkdir()

    timed_out = run_hidden_verifier(
        "opaque-check",
        workspace,
        timeout_seconds=0.01,
    )
    cancelled = run_hidden_verifier(
        "opaque-check",
        workspace,
        timeout_seconds=5,
        cancel_requested=lambda: True,
    )

    assert timed_out.exit_code == 124
    assert cancelled.exit_code == 130


def test_bundled_baseline_verifier_emits_json_without_writing_workspace(
    tmp_path: Path,
) -> None:
    source = Path(__file__).resolve().parents[1] / "forgebench" / "forgebench-monorepo"
    workspace = tmp_path / "workspace"
    import shutil

    shutil.copytree(source, workspace)
    before = {
        path.relative_to(workspace).as_posix(): path.read_bytes()
        for path in workspace.rglob("*")
        if path.is_file()
    }

    output = run_hidden_verifier(
        "baseline-health",
        workspace,
        timeout_seconds=5,
    )

    after = {
        path.relative_to(workspace).as_posix(): path.read_bytes()
        for path in workspace.rglob("*")
        if path.is_file()
    }
    assert output.exit_code == 0
    assert json.loads(output.stdout)["ok"] is True
    assert after == before
