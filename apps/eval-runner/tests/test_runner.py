import sys
from pathlib import Path

from app.models import EvaluationTask, FailureCategory
from app.runner import DeterministicMockRunner, ForgeAgentRunner, create_runner


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


def test_mock_runner_applies_metadata_for_scope_and_multi_step_trace() -> None:
    task = EvaluationTask(
        id="forbidden-file-change",
        title="Forbidden file change",
        prompt="Only change src/app.py.",
        context_files=["src/app.py"],
        verification_command="pytest",
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[".env"],
        metadata={
            "mock": {
                "changed_files": ["src/app.py", ".env"],
                "raw_events": [
                    {"event_type": "model_round", "round": 1},
                    {"event_type": "tool_call_start", "tool_name": "read_file"},
                    {"event_type": "tool_call_start", "tool_name": "edit_file"},
                ],
                "tool_commands": ["read_file src/app.py", "edit_file src/app.py", "edit_file .env"],
                "model_rounds": 3,
                "confirm_requests": 1,
                "input_tokens": 240,
                "output_tokens": 80,
            }
        },
    )

    trace = DeterministicMockRunner().run_task(task)

    assert trace.changed_files == ["src/app.py", ".env"]
    assert trace.scope_violations == ["forbidden_change:.env", "unexpected_change:.env"]
    assert trace.error == "scope_violation"
    assert trace.failure_category == FailureCategory.SCOPE_VIOLATION
    assert trace.model_rounds == 3
    assert trace.confirm_requests == 1
    assert trace.input_tokens == 240
    assert trace.output_tokens == 80
    assert [call.command for call in trace.tool_calls] == [
        "read_file src/app.py",
        "edit_file src/app.py",
        "edit_file .env",
    ]
    assert trace.raw_events[0]["event_type"] == "model_round"


def test_mock_runner_can_simulate_timeout_or_runner_error() -> None:
    task = EvaluationTask(
        id="timeout-or-runner-error",
        title="Timeout or runner error",
        prompt="Simulate a task that cannot complete.",
        verification_command="pytest",
        metadata={
            "mock": {
                "error": "timeout",
                "failure_category": "timeout",
                "failure_reason": "Mock runner exceeded max_duration_seconds.",
                "changed_files": [],
            }
        },
    )

    trace = DeterministicMockRunner().run_task(task)

    assert trace.verification_result is None
    assert trace.changed_files == []
    assert trace.error == "timeout"
    assert trace.failure_category == FailureCategory.TIMEOUT
    assert trace.failure_reason == "Mock runner exceeded max_duration_seconds."


def test_runner_factory_selects_mock_and_forge_runners(tmp_path: Path) -> None:
    command = [sys.executable, str(tmp_path / "forge_agent.py")]

    assert isinstance(
        create_runner(provider="mock", model="deterministic-agent-v1"),
        DeterministicMockRunner,
    )
    assert isinstance(
        create_runner(provider="forge", model="local-forge", forge_command=command),
        ForgeAgentRunner,
    )


def test_forge_runner_converts_external_trace_payload(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")
    script = tmp_path / "fake_forge_agent.py"
    script.write_text(
        """
import json
import pathlib
import sys

payload = json.loads(sys.stdin.read())
workspace = pathlib.Path(payload["workspace_path"])
assert (workspace / "src" / "app.py").exists()
json.dump(
    {
        "raw_events": [
            {"event_type": "tool_call_start", "tool_name": "read_file"},
            {"event_type": "tool_call_result", "result": "ok"},
        ],
        "tool_calls": [
            {
                "command": "read_file src/app.py",
                "stdout": "loaded",
                "exit_code": 0,
                "duration_ms": 18,
            }
        ],
        "shell_outputs": [
            {"command": "pytest", "stdout": "1 passed", "exit_code": 0, "duration_ms": 120}
        ],
        "file_diffs": [
            {
                "path": "src/app.py",
                "change_type": "modified",
                "diff": "diff --git a/src/app.py b/src/app.py",
            }
        ],
        "changed_files": ["src/app.py"],
        "verification_result": {
            "command": "pytest",
            "passed": True,
            "stdout": "1 passed",
            "stderr": "",
            "exit_code": 0,
            "duration_ms": 120,
        },
        "final_answer": "Forge completed the task.",
        "model_rounds": 2,
        "confirm_requests": 1,
        "input_tokens": 120,
        "output_tokens": 40,
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="forge-real-task",
        title="Run Forge task",
        prompt="Modify src/app.py and verify.",
        fixture_path=str(fixture),
        context_files=["src/app.py"],
        verification_command="pytest",
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[".env"],
    )
    runner = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    )

    trace = runner.run_task(task)

    assert trace.provider == "forge"
    assert trace.model == "local-forge"
    assert trace.raw_events[0]["event_type"] == "tool_call_start"
    assert trace.tool_calls[0].command == "read_file src/app.py"
    assert trace.shell_outputs[0].command == "pytest"
    assert trace.changed_files == ["src/app.py"]
    assert trace.scope_violations == []
    assert trace.verification_result is not None
    assert trace.verification_result.passed is True
    assert trace.model_rounds == 2
    assert trace.confirm_requests == 1
    assert trace.input_tokens == 120
    assert trace.output_tokens == 40
    assert trace.failure_category == FailureCategory.NONE


def test_forge_runner_reports_scope_violations_from_changed_files(tmp_path: Path) -> None:
    script = tmp_path / "fake_scope_violation.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": ["src/app.py", ".env"],
        "verification_result": {
            "command": "pytest",
            "passed": True,
            "stdout": "1 passed",
            "stderr": "",
            "exit_code": 0,
            "duration_ms": 120,
        },
        "final_answer": "Done.",
        "headless_continuity_formed_count": 1,
        "headless_continuity_error": None,
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="scope-check",
        title="Scope check",
        prompt="Only change src/app.py.",
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[".env"],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "scope_violation"
    assert trace.failure_category == FailureCategory.SCOPE_VIOLATION
    assert trace.scope_violations == ["forbidden_change:.env", "unexpected_change:.env"]


def test_forge_runner_executes_task_validation_commands(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")
    script = tmp_path / "fake_without_verification.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": ["src/app.py"],
        "final_answer": "Done.",
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="validation-command",
        title="Validation command",
        prompt="Modify src/app.py.",
        fixture_path=str(fixture),
        expected_files_changed=["src/app.py"],
        validation_commands=[
            f"{sys.executable} -c \"from pathlib import Path; assert Path('src/app.py').exists()\""
        ],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.passed is True
    assert trace.verification_result.command == task.validation_commands[0]
    assert trace.shell_outputs[-1].command == task.validation_commands[0]


def test_forge_runner_executes_post_validation_commands_after_headless_validation(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")
    script = tmp_path / "fake_with_continuity_db.py"
    script.write_text(
        """
import json
import pathlib
import sqlite3
import sys

payload = json.loads(sys.stdin.read())
workspace = pathlib.Path(payload["workspace_path"])
(workspace / ".forge").mkdir(exist_ok=True)
db_path = workspace / ".forge" / "continuity.db"
conn = sqlite3.connect(db_path)
conn.execute("CREATE TABLE continuity_events (event_type TEXT NOT NULL, event_json TEXT NOT NULL)")
conn.execute("CREATE TABLE continuity_experiences (id TEXT PRIMARY KEY)")
conn.execute("CREATE TABLE continuity_formed_reflections (session_id TEXT)")
conn.execute(
    "INSERT INTO continuity_events (event_type, event_json) VALUES (?, ?)",
    (
        "reflection",
        json.dumps(
            {
                "reflection": {
                    "session_id": "session-1",
                    "episode": {
                        "changed_files": ["src/app.py"],
                        "tool_count": 2,
                        "failed_tools": 0,
                    },
                }
            }
        ),
    ),
)
conn.execute("INSERT INTO continuity_experiences (id) VALUES ('experience-1')")
conn.execute("INSERT INTO continuity_formed_reflections (session_id) VALUES ('session-1')")
conn.commit()
conn.close()
json.dump(
    {
        "changed_files": ["src/app.py"],
        "verification_result": {
            "command": "pytest",
            "passed": True,
            "stdout": "1 passed",
            "stderr": "",
            "exit_code": 0,
            "duration_ms": 120,
        },
        "final_answer": "Done.",
        "headless_continuity_formed_count": 1,
        "headless_continuity_error": None,
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    post_command = (
        f"{sys.executable} -c \"from pathlib import Path; "
        "assert Path('.forge/continuity.db').exists(); print('continuity ok')\""
    )
    task = EvaluationTask(
        id="continuity-post-validation",
        title="Continuity post validation",
        prompt="Modify src/app.py.",
        fixture_path=str(fixture),
        expected_files_changed=["src/app.py"],
        validation_commands=[f"{sys.executable} -c \"print('app ok')\""],
        post_validation_commands=[post_command],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.passed is True
    assert trace.verification_result.command == post_command
    assert [output.command for output in trace.shell_outputs[-2:]] == [
        task.validation_commands[0],
        post_command,
    ]
    assert trace.shell_outputs[-1].stdout == "continuity ok\n"
    headless_diagnostic = next(
        event
        for event in trace.raw_events
        if event["event_type"] == "eval_headless_continuity_diagnostic"
    )
    assert headless_diagnostic["formed_count"] == 1
    assert headless_diagnostic["error"] is None
    diagnostic = next(
        event
        for event in trace.raw_events
        if event["event_type"] == "eval_continuity_db_diagnostic"
    )
    assert diagnostic["exists"] is True
    assert diagnostic["event_counts"] == {"reflection": 1}
    assert diagnostic["experience_count"] == 1
    assert diagnostic["formed_reflection_count"] == 1
    assert diagnostic["reflection_episodes"] == [
        {
            "user_goal_summary": None,
            "changed_files": ["src/app.py"],
            "file_changes_count": 0,
            "file_changes": [],
            "tool_count": 2,
            "failed_tools": 0,
            "notable_failures": [],
            "outcome": None,
            "verification_status": None,
        }
    ]


def test_forge_runner_reports_first_failed_validation_command(tmp_path: Path) -> None:
    script = tmp_path / "fake_success.py"
    script.write_text(
        """
import json
import sys

json.dump({"changed_files": ["src/app.py"], "final_answer": "Done."}, sys.stdout)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="failed-post-validation",
        title="Failed post validation",
        prompt="Modify src/app.py.",
        expected_files_changed=["src/app.py"],
        post_validation_commands=[
            f"{sys.executable} -c \"import sys; print('app failed'); sys.exit(2)\"",
            f"{sys.executable} -c \"print('continuity ok')\"",
        ],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.passed is False
    assert trace.verification_result.exit_code == 2
    assert trace.verification_result.stdout == "app failed\n"
    assert trace.error == "verification_failed"


def test_forge_runner_clears_recovered_verification_failure_after_validation(
    tmp_path: Path,
) -> None:
    script = tmp_path / "fake_recovered_verification.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": ["src/app.py"],
        "verification_result": {
            "command": "npm test",
            "passed": False,
            "stdout": "failed once",
            "stderr": "",
            "exit_code": 1,
            "duration_ms": 120,
        },
        "error": "verification_failed",
        "failure_category": "verification_failed",
        "failure_reason": "Verification command failed: npm test",
        "final_answer": "Done.",
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="recovered-verification",
        title="Recovered verification",
        prompt="Modify src/app.py.",
        expected_files_changed=["src/app.py"],
        validation_commands=[f"{sys.executable} -c \"print('ok')\""],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.passed is True
    assert trace.error is None
    assert trace.failure_category == FailureCategory.NONE
    assert trace.failure_reason is None


def test_forge_runner_normalizes_legacy_verification_failure_category(tmp_path: Path) -> None:
    script = tmp_path / "fake_legacy_verification.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": ["src/app.py"],
        "verification_result": {
            "command": "pytest",
            "passed": False,
            "stdout": "failed",
            "stderr": "",
            "exit_code": 1,
            "duration_ms": 120,
        },
        "error": "verification",
        "failure_category": "verification",
        "failure_reason": "Verification command failed.",
        "final_answer": "Done.",
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="legacy-verification",
        title="Legacy verification",
        prompt="Modify src/app.py.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "verification_failed"
    assert trace.failure_category == FailureCategory.VERIFICATION_FAILED
    assert trace.failure_reason == "Verification command failed."


def test_forge_runner_maps_headless_agent_error_to_runner_error(tmp_path: Path) -> None:
    script = tmp_path / "fake_headless_agent_error.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": [],
        "error": "agent_error",
        "failure_category": "agent_error",
        "failure_reason": "Forge agent turn failed.",
        "final_answer": "",
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="headless-agent-error",
        title="Headless agent error",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "agent_error"
    assert trace.failure_category == FailureCategory.RUNNER_ERROR
    assert trace.failure_reason == "Forge agent turn failed."


def test_forge_runner_reports_invalid_stdout_as_contract_error(tmp_path: Path) -> None:
    script = tmp_path / "fake_invalid_stdout.py"
    script.write_text("print('not json')\n", encoding="utf-8")
    task = EvaluationTask(
        id="invalid-stdout",
        title="Invalid stdout",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "invalid_forge_trace"
    assert trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR
    assert trace.failure_reason.startswith("Forge command returned invalid trace JSON:")


def test_forge_runner_returns_runner_error_when_command_is_missing() -> None:
    task = EvaluationTask(id="missing-command", title="Missing command", prompt="Run Forge.")

    trace = ForgeAgentRunner(provider="forge", model="local-forge", command=None).run_task(task)

    assert trace.error == "forge_command_not_configured"
    assert trace.failure_category == FailureCategory.RUNNER_ERROR
    assert trace.failure_reason == "Set FORGE_EVAL_FORGE_AGENT_COMMAND to run provider=forge."
