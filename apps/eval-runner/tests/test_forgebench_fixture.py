import json
import shutil
import subprocess
import sys
from pathlib import Path

from app.artifact_hashing import IGNORED_TREE_PARTS, canonical_tree_digest

FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
FIXTURE_LOCK = FORGEBENCH_ROOT / "fixture-lock.json"


def test_forgebench_fixture_has_exact_product_shape() -> None:
    assert FIXTURE.is_dir()
    assert not (FIXTURE / ".git").exists()
    assert {path.name for path in FIXTURE.iterdir() if path.is_dir()} == {
        "apps",
        "packages",
        "services",
        "tests",
        "tools",
    }
    assert {path.name for path in FIXTURE.iterdir() if path.is_file()} == {
        "README.md",
        "package.json",
        "pyproject.toml",
        "tsconfig.json",
    }

    package = json.loads((FIXTURE / "package.json").read_text(encoding="utf-8"))
    assert package["name"] == "forgebench-monorepo"
    assert package["version"] == "1.0.0"
    assert package["workspaces"] == ["apps/*", "packages/*"]

    pyproject = (FIXTURE / "pyproject.toml").read_text(encoding="utf-8")
    assert "services/task-api" in pyproject
    assert "services/event-worker" in pyproject


def test_forgebench_fixture_has_no_generated_or_cache_directories() -> None:
    generated = {
        path.relative_to(FIXTURE).as_posix()
        for path in FIXTURE.rglob("*")
        if path.is_dir() and path.name in IGNORED_TREE_PARTS
    }

    assert generated == set()


def test_forgebench_fixture_passes_full_verification_from_clean_copy(
    tmp_path: Path,
) -> None:
    workspace = tmp_path / "workspace"
    shutil.copytree(FIXTURE, workspace)

    completed = subprocess.run(
        [sys.executable, "tools/release/verify.py"],
        cwd=workspace,
        text=True,
        capture_output=True,
        timeout=120,
        check=False,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_forgebench_fixture_lock_matches_canonical_tree_digest() -> None:
    lock = json.loads(FIXTURE_LOCK.read_text(encoding="utf-8"))

    assert set(lock) == {
        "schema_version",
        "fixture_id",
        "fixture_version",
        "canonical_tree_sha256",
        "verification_command",
    }
    assert lock["schema_version"] == 1
    assert lock["fixture_id"] == "forgebench-monorepo"
    assert lock["fixture_version"] == "1.0.0"
    assert lock["verification_command"] == "python3 tools/release/verify.py"
    assert lock["canonical_tree_sha256"] == canonical_tree_digest(FIXTURE)
