from datetime import UTC, datetime

from app.models import EvaluationTask, FileDiff


def utc_now() -> datetime:
    return datetime.now(UTC)


def duration_ms(started_at: datetime, ended_at: datetime) -> int:
    elapsed = (ended_at - started_at).total_seconds() * 1000
    return max(0, int(elapsed))


def build_mock_file_diff(task: EvaluationTask) -> FileDiff:
    path = task.context_files[0] if task.context_files else "workspace/changes.patch"
    return FileDiff(
        path=path,
        change_type="modified",
        diff=(
            f"diff --git a/{path} b/{path}\n"
            f"--- a/{path}\n"
            f"+++ b/{path}\n"
            "@@ -1,3 +1,4 @@\n"
            "+# Deterministic mock change produced by forge-eval-runner\n"
        ),
    )
