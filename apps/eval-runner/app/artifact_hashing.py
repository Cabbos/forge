import hashlib
import json
import os
import stat
from collections.abc import Iterator
from pathlib import Path

IGNORED_TREE_PARTS = frozenset(
    {
        ".git",
        "node_modules",
        ".venv",
        "__pycache__",
        ".pytest_cache",
        "dist",
        "coverage",
    }
)
MISSING_TREE_DIGEST = "missing"


def canonical_tree_manifest(root: Path) -> list[dict[str, str | int]]:
    if not root.is_dir():
        raise ValueError(f"artifact tree is not a directory: {root}")

    entries: list[dict[str, str | int]] = []
    for path in _iter_tree_paths(root):
        relative = path.relative_to(root)
        if path.is_symlink():
            kind = "symlink"
            content = os.fsencode(os.readlink(path))
        elif path.is_dir():
            continue
        else:
            kind = "file"
            content = path.read_bytes()
        entries.append(
            {
                "path": relative.as_posix(),
                "kind": kind,
                "mode": stat.S_IMODE(path.lstat().st_mode),
                "sha256": hashlib.sha256(content).hexdigest(),
            }
        )
    return entries


def _iter_tree_paths(root: Path) -> Iterator[Path]:
    """Yield artifact paths without descending into ignored trees.

    ``Path.rglob`` first walks every directory and only lets callers filter
    the resulting paths.  That is unsafe for live Git worktrees: Git may pack
    or prune ``.git/objects`` between directory enumeration and stat, causing
    an otherwise unrelated digest to fail with ``FileNotFoundError``.  Prune
    ignored directories at the directory boundary instead.
    """

    def visit(directory: Path) -> Iterator[Path]:
        try:
            children = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except FileNotFoundError:
            return
        for child in children:
            if child.name in IGNORED_TREE_PARTS:
                continue
            path = Path(child.path)
            if child.is_symlink():
                yield path
            elif child.is_dir(follow_symlinks=False):
                yield from visit(path)
            else:
                yield path

    yield from visit(root)


def canonical_manifest_digest(manifest: object) -> str:
    payload = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def canonical_tree_digest(root: Path) -> str:
    return canonical_manifest_digest(canonical_tree_manifest(root))


def tree_digest_or_none(path: str | None) -> str | None:
    if path is None:
        return None
    root = Path(path)
    if not root.is_dir():
        return MISSING_TREE_DIGEST
    return canonical_tree_digest(root)
