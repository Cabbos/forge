import subprocess
from pathlib import Path

from app.models import FileDiff, WorkspaceCheck


def replay_patch(workspace: Path, diffs: list[FileDiff]) -> WorkspaceCheck:
    patch_text = "\n".join(diff.diff for diff in diffs)
    completed = subprocess.run(
        ["patch", "-p1"],
        cwd=workspace,
        input=patch_text,
        text=True,
        capture_output=True,
        check=False,
    )
    return WorkspaceCheck(
        ok=completed.returncode == 0,
        message=completed.stderr or completed.stdout,
    )
