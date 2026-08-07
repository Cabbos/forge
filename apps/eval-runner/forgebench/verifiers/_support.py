import hashlib
import json
import os
import subprocess
import sys
import tempfile
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any

OUTPUT_LIMIT = 12_000
IGNORED_PARTS = frozenset(
    {".git", "node_modules", ".venv", "__pycache__", ".pytest_cache", "dist", "coverage"}
)


def workspace_from_argv() -> Path:
    if len(sys.argv) != 2:
        raise ValueError("hidden verifier requires exactly one workspace argument")
    workspace = Path(sys.argv[1]).resolve()
    if not workspace.is_dir():
        raise ValueError("hidden verifier workspace does not exist")
    return workspace


def assert_workspace_path(workspace: Path, relative_path: str) -> Path:
    target = (workspace / relative_path).resolve()
    if not target.is_relative_to(workspace):
        raise ValueError(f"path escapes workspace: {relative_path}")
    return target


def run_typescript_test(workspace: Path, source: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="forgebench-hidden-ts-") as temporary:
        test_path = Path(temporary) / "hidden.test.ts"
        test_path.write_text(source, encoding="utf-8")
        return run_read_only_command(
            workspace,
            ["bun", "test", str(test_path)],
            extra_env={"FORGEBENCH_WORKSPACE": str(workspace)},
        )


def run_python_test(workspace: Path, source: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="forgebench-hidden-py-") as temporary:
        test_path = Path(temporary) / "test_hidden.py"
        test_path.write_text(source, encoding="utf-8")
        return run_read_only_command(
            workspace,
            [
                "uv",
                "run",
                "--no-project",
                "--with",
                "pytest==9.0.3",
                "pytest",
                "-p",
                "no:cacheprovider",
                str(test_path),
                "-q",
            ],
            extra_env={
                "FORGEBENCH_WORKSPACE": str(workspace),
                "PYTHONDONTWRITEBYTECODE": "1",
            },
        )


def run_read_only_command(
    workspace: Path,
    command: Sequence[str],
    *,
    extra_env: dict[str, str] | None = None,
) -> dict[str, Any]:
    before = workspace_manifest(workspace)
    environment = os.environ.copy()
    environment.update(extra_env or {})
    completed = subprocess.run(
        list(command),
        cwd=workspace,
        env=environment,
        text=True,
        capture_output=True,
        timeout=120,
        check=False,
    )
    if workspace_manifest(workspace) != before:
        raise AssertionError("hidden verifier command modified the workspace")
    result = {
        "exit_code": completed.returncode,
        "stdout": completed.stdout[:OUTPUT_LIMIT],
        "stderr": completed.stderr[:OUTPUT_LIMIT],
    }
    if completed.returncode != 0:
        raise AssertionError(json.dumps(result, sort_keys=True))
    return result


def workspace_manifest(workspace: Path) -> list[tuple[str, str]]:
    entries: list[tuple[str, str]] = []
    for path in sorted(
        workspace.rglob("*"),
        key=lambda item: item.relative_to(workspace).as_posix(),
    ):
        relative = path.relative_to(workspace)
        if any(part in IGNORED_PARTS for part in relative.parts) or path.is_dir():
            continue
        content = os.fsencode(os.readlink(path)) if path.is_symlink() else path.read_bytes()
        entries.append((relative.as_posix(), hashlib.sha256(content).hexdigest()))
    return entries


def verify(
    verifier_id: str,
    assertion: Callable[[Path], dict[str, Any] | None],
) -> None:
    try:
        workspace = workspace_from_argv()
        before = workspace_manifest(workspace)
        detail = assertion(workspace) or {}
        if workspace_manifest(workspace) != before:
            raise AssertionError("hidden verifier modified the workspace")
        print(json.dumps({"ok": True, "verifier_id": verifier_id, **detail}, sort_keys=True))
    except Exception as exc:
        print(
            json.dumps(
                {
                    "ok": False,
                    "verifier_id": verifier_id,
                    "error": f"{type(exc).__name__}: {exc}",
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1) from exc
