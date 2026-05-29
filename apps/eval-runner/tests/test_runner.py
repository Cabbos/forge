from app.models import EvaluationTask
from app.runner import DeterministicMockRunner


def test_mock_runner_creates_complete_agent_trace_for_passing_task() -> None:
    task = EvaluationTask(
        id="add-cli-flag",
        title="Add CLI flag",
        prompt="Add a --dry-run flag and verify tests.",
        context_files=["src/cli.py", "tests/test_cli.py"],
        verification_command="pytest tests/test_cli.py",
        expected_success=True,
    )
    runner = DeterministicMockRunner(provider="mock", model="deterministic-agent-v1")

    trace = runner.run_task(task)

    assert trace.task_id == "add-cli-flag"
    assert trace.user_prompt == task.prompt
    assert trace.provider == "mock"
    assert trace.model == "deterministic-agent-v1"
    assert trace.context_files == ["src/cli.py", "tests/test_cli.py"]
    assert len(trace.tool_calls) == 2
    assert trace.shell_outputs[0].command == "pytest tests/test_cli.py"
    assert trace.file_diffs[0].path == "src/cli.py"
    assert "--- a/src/cli.py" in trace.file_diffs[0].diff
    assert "+++ b/src/cli.py" in trace.file_diffs[0].diff
    assert trace.final_answer.startswith("Mock agent completed")
    assert trace.verification_result is not None
    assert trace.verification_result.passed is True
    assert trace.failure_reason is None
    assert trace.duration_ms >= 0
    assert trace.model_dump(mode="json")["started_at"].endswith("+00:00")


def test_mock_runner_records_verification_failure_reason() -> None:
    task = EvaluationTask(
        id="fix-bug",
        title="Fix failing parser",
        prompt="Fix parser regression.",
        context_files=["src/parser.py"],
        verification_command="pytest tests/test_parser.py",
        expected_success=False,
    )
    runner = DeterministicMockRunner()

    trace = runner.run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.passed is False
    assert trace.error == "verification_failed"
    assert trace.failure_reason == "Mock verification command returned a non-zero exit code."
    assert trace.failure_category.value == "verification_failed"
