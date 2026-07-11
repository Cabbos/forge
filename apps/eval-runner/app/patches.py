from pathlib import Path

from app.models import FileDiff, ProcessOutcome, WorkspaceCheck
from app.process_control import CancelRequested, never_cancelled, run_bounded_process


def replay_patch(
    workspace: Path,
    diffs: list[FileDiff],
    *,
    timeout_seconds: float = 300.0,
    cancel_requested: CancelRequested = never_cancelled,
) -> WorkspaceCheck:
    patch_text = "\n".join(diff.diff for diff in diffs)
    completed = run_bounded_process(
        ["patch", "-p1"],
        cwd=workspace,
        input_text=patch_text,
        timeout_seconds=timeout_seconds,
        cancel_requested=cancel_requested,
    )
    return WorkspaceCheck(
        ok=(completed.outcome == ProcessOutcome.COMPLETED and completed.returncode == 0),
        message=(
            f"patch replay {completed.outcome.value}: {completed.stderr or completed.stdout}"
            if completed.outcome != ProcessOutcome.COMPLETED
            else completed.stderr or completed.stdout
        ),
    )
