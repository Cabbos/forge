import os
from pathlib import Path

import pytest

from app.workspace_observer import observe_workspace_changes, snapshot_workspace


def test_workspace_observer_reports_added_modified_and_deleted_files(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "modify.txt").write_text("before\n", encoding="utf-8")
    (workspace / "delete.txt").write_text("delete\n", encoding="utf-8")
    before = snapshot_workspace(workspace)

    (workspace / "modify.txt").write_text("after\n", encoding="utf-8")
    (workspace / "delete.txt").unlink()
    (workspace / "add.txt").write_text("add\n", encoding="utf-8")

    observation = observe_workspace_changes(before, workspace, reported_changed_files=[])

    assert observation.available is True
    assert observation.added_files == ["add.txt"]
    assert observation.modified_files == ["modify.txt"]
    assert observation.deleted_files == ["delete.txt"]
    assert observation.changed_files == ["add.txt", "delete.txt", "modify.txt"]


def test_workspace_observer_hashes_binary_and_symlink_targets(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "binary.bin").write_bytes(b"\x00\x01")
    (workspace / "target-a").write_text("a", encoding="utf-8")
    (workspace / "target-b").write_text("b", encoding="utf-8")
    (workspace / "link").symlink_to("target-a")
    before = snapshot_workspace(workspace)

    (workspace / "binary.bin").write_bytes(b"\x00\x02")
    (workspace / "link").unlink()
    (workspace / "link").symlink_to("target-b")

    observation = observe_workspace_changes(before, workspace, reported_changed_files=[])

    assert observation.modified_files == ["binary.bin", "link"]


def test_workspace_observer_records_report_mismatch(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    before = snapshot_workspace(workspace)
    (workspace / "actual.txt").write_text("x", encoding="utf-8")

    observation = observe_workspace_changes(
        before,
        workspace,
        reported_changed_files=["claimed.txt"],
    )

    assert observation.changed_files == ["actual.txt"]
    assert observation.reported_changed_files == ["claimed.txt"]
    assert observation.mismatch_files == ["actual.txt", "claimed.txt"]


def test_workspace_snapshot_is_ordered_and_excludes_git_metadata(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "z.txt").write_text("z", encoding="utf-8")
    (workspace / "a.txt").write_text("a", encoding="utf-8")
    (workspace / ".git").mkdir()
    (workspace / ".git" / "config").write_text("secret", encoding="utf-8")

    snapshot = snapshot_workspace(workspace)

    assert list(snapshot) == ["a.txt", "z.txt"]
    assert ".git/config" not in snapshot


def test_workspace_observer_marks_snapshot_errors_unavailable(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()

    def fail_snapshot(_: Path) -> object:
        raise OSError("observer unavailable")

    monkeypatch.setattr("app.workspace_observer.snapshot_workspace", fail_snapshot)

    observation = observe_workspace_changes(
        {},
        workspace,
        reported_changed_files=["claimed.txt", "claimed.txt"],
    )

    assert observation.available is False
    assert observation.source == "filesystem_snapshot"
    assert observation.reported_changed_files == ["claimed.txt"]
    assert observation.error == "OSError: observer unavailable"


@pytest.mark.skipif(not hasattr(os, "symlink"), reason="symlinks unavailable")
def test_workspace_snapshot_records_symlink_itself_not_target_contents(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "target").write_text("one", encoding="utf-8")
    (workspace / "link").symlink_to("target")

    before = snapshot_workspace(workspace)
    (workspace / "target").write_text("two", encoding="utf-8")
    after = snapshot_workspace(workspace)

    assert before["link"] == after["link"]
    assert before["target"] != after["target"]
