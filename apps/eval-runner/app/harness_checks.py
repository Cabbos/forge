from pathlib import Path

from app.cases import load_cases
from app.metrics import trace_passed
from app.models import WorkspaceCheck
from app.runner import DeterministicMockRunner


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
    return WorkspaceCheck(
        ok=not failed,
        modified_files=failed,
        message=None if not failed else "Golden cases failed: " + ", ".join(failed),
    )
