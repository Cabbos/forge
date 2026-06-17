from pathlib import Path

from app.cases import load_cases
from app.runner import DeterministicMockRunner


def run_golden_harness_check(cases_path: Path) -> bool:
    tasks = load_cases(cases_path)
    runner = DeterministicMockRunner()
    traces = [runner.run_task(task) for task in tasks if task.expected_success]
    return all(
        trace.verification_result is not None and trace.verification_result.passed
        for trace in traces
    )
