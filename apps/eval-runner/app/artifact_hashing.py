import hashlib
import json
import os
import stat
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
    paths = sorted(root.rglob("*"), key=lambda path: path.relative_to(root).as_posix())
    for path in paths:
        relative = path.relative_to(root)
        if any(part in IGNORED_TREE_PARTS for part in relative.parts):
            continue
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
