import tempfile
from pathlib import Path

from app.cases import load_cases
from app.metrics import trace_passed
from app.models import FileDiff, WorkspaceCheck
from app.patches import replay_patch
from app.runner import DeterministicMockRunner
from app.workspace_observer import observe_workspace_changes, snapshot_workspace

GOLDEN_PATCH = """diff --git a/src/value.txt b/src/value.txt
--- a/src/value.txt
+++ b/src/value.txt
@@ -1 +1 @@
-before
+after
"""


def run_observer_patch_replay_self_check() -> WorkspaceCheck:
    with tempfile.TemporaryDirectory(prefix="forge-harness-golden-") as raw_root:
        root = Path(raw_root)
        workspace = root / "workspace"
        replay = root / "replay"
        for target in (workspace, replay):
            (target / "src").mkdir(parents=True)
            (target / "src" / "value.txt").write_text("before\n", encoding="utf-8")

        before = snapshot_workspace(workspace)
        (workspace / "src" / "value.txt").write_text("after\n", encoding="utf-8")
        observed = observe_workspace_changes(
            before,
            workspace,
            reported_changed_files=["src/value.txt"],
        )
        replayed = replay_patch(
            replay,
            [FileDiff(path="src/value.txt", diff=GOLDEN_PATCH)],
            timeout_seconds=10,
        )
        replay_value = (replay / "src" / "value.txt").read_text(encoding="utf-8")
        ok = (
            observed.available
            and observed.changed_files == ["src/value.txt"]
            and observed.mismatch_files == []
            and replayed.ok
            and replay_value == "after\n"
        )
        return WorkspaceCheck(
            ok=ok,
            untracked_files=observed.mismatch_files,
            modified_files=observed.changed_files,
            message=(
                "observer and patch replay self-check passed"
                if ok
                else "observer or patch replay self-check failed"
            ),
        )


def run_golden_harness_check(cases_path: Path) -> WorkspaceCheck:
    tasks = load_cases(cases_path)
    golden_tasks = [
        task
        for task in tasks
        if task.expected_success and not bool(task.metadata.get("contract_only"))
    ]
    if not golden_tasks:
        return WorkspaceCheck(
            ok=False,
            message="No executable expected-success golden cases found.",
        )
    traces = [DeterministicMockRunner().run_task(task) for task in golden_tasks]
    failed = [trace.task_id for trace in traces if not trace_passed(trace)]
    if failed:
        return WorkspaceCheck(
            ok=False,
            modified_files=failed,
            message="Golden cases failed: " + ", ".join(failed),
        )
    observer_replay = run_observer_patch_replay_self_check()
    if not observer_replay.ok:
        return observer_replay
    return WorkspaceCheck(
        ok=True,
        message="deterministic contract, observer, and patch replay checks passed",
    )
