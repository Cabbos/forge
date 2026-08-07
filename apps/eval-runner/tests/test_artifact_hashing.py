import os
from pathlib import Path

from app.artifact_hashing import canonical_tree_digest


def write_tree(root: Path) -> None:
    (root / "src").mkdir(parents=True)
    (root / "src" / "app.ts").write_text("export const value = 1;\n", encoding="utf-8")
    script = root / "verify.sh"
    script.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    script.chmod(0o755)
    (root / "current").symlink_to("src/app.ts")


def test_equal_trees_at_different_absolute_paths_have_same_digest(tmp_path: Path) -> None:
    first = tmp_path / "first"
    second = tmp_path / "elsewhere" / "second"
    write_tree(first)
    write_tree(second)

    assert canonical_tree_digest(first) == canonical_tree_digest(second)


def test_tree_digest_tracks_content_mode_symlink_target_and_relative_path(
    tmp_path: Path,
) -> None:
    baseline = tmp_path / "baseline"
    write_tree(baseline)
    baseline_digest = canonical_tree_digest(baseline)

    content_changed = tmp_path / "content"
    write_tree(content_changed)
    (content_changed / "src" / "app.ts").write_text("export const value = 2;\n", encoding="utf-8")

    mode_changed = tmp_path / "mode"
    write_tree(mode_changed)
    (mode_changed / "verify.sh").chmod(0o644)

    symlink_changed = tmp_path / "symlink"
    write_tree(symlink_changed)
    (symlink_changed / "other.ts").write_text("export {};\n", encoding="utf-8")
    (symlink_changed / "current").unlink()
    (symlink_changed / "current").symlink_to("other.ts")

    path_changed = tmp_path / "path"
    write_tree(path_changed)
    (path_changed / "src" / "app.ts").rename(path_changed / "src" / "renamed.ts")

    assert canonical_tree_digest(content_changed) != baseline_digest
    assert canonical_tree_digest(mode_changed) != baseline_digest
    assert canonical_tree_digest(symlink_changed) != baseline_digest
    assert canonical_tree_digest(path_changed) != baseline_digest


def test_tree_digest_excludes_generated_and_cache_directories(tmp_path: Path) -> None:
    root = tmp_path / "tree"
    write_tree(root)
    baseline_digest = canonical_tree_digest(root)

    for ignored in (
        ".git",
        "node_modules",
        ".venv",
        "__pycache__",
        ".pytest_cache",
        "dist",
        "coverage",
    ):
        artifact = root / "nested" / ignored / "artifact.bin"
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(os.urandom(16))

    assert canonical_tree_digest(root) == baseline_digest
