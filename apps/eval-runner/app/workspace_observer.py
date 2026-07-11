from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass
from pathlib import Path

from app.models import WorkspaceObservation


@dataclass(frozen=True)
class FileState:
    kind: str
    digest: str
    size_bytes: int


WorkspaceSnapshot = dict[str, FileState]


def snapshot_workspace(workspace: Path) -> WorkspaceSnapshot:
    snapshot: WorkspaceSnapshot = {}
    for path in sorted(workspace.rglob("*")):
        relative = path.relative_to(workspace)
        if ".git" in relative.parts:
            continue

        key = relative.as_posix()
        if path.is_symlink():
            target_bytes = os.fsencode(os.readlink(path))
            snapshot[key] = FileState(
                kind="symlink",
                digest=hashlib.sha256(target_bytes).hexdigest(),
                size_bytes=len(target_bytes),
            )
            continue
        if path.is_dir():
            continue

        digest = hashlib.sha256()
        size_bytes = 0
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
                size_bytes += len(chunk)
        snapshot[key] = FileState(
            kind="file",
            digest=digest.hexdigest(),
            size_bytes=size_bytes,
        )
    return snapshot


def observe_workspace_changes(
    before: WorkspaceSnapshot,
    workspace: Path,
    *,
    reported_changed_files: list[str],
) -> WorkspaceObservation:
    reported = sorted(set(reported_changed_files))
    try:
        after = snapshot_workspace(workspace)
    except OSError as exc:
        return WorkspaceObservation(
            available=False,
            source="filesystem_snapshot",
            reported_changed_files=reported,
            error=f"{type(exc).__name__}: {exc}",
        )

    before_paths = set(before)
    after_paths = set(after)
    added = sorted(after_paths - before_paths)
    deleted = sorted(before_paths - after_paths)
    modified = sorted(
        path for path in before_paths & after_paths if before[path] != after[path]
    )
    changed = sorted([*added, *deleted, *modified])
    mismatch = sorted(set(changed).symmetric_difference(reported))

    return WorkspaceObservation(
        available=True,
        source="filesystem_snapshot",
        changed_files=changed,
        added_files=added,
        modified_files=modified,
        deleted_files=deleted,
        reported_changed_files=reported,
        mismatch_files=mismatch,
    )
