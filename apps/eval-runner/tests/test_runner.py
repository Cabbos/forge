import sqlite3
import subprocess
import sys
import threading
from pathlib import Path

import pytest
from pydantic import ValidationError

from app.models import (
    AgentTrace,
    EvalProvider,
    EvaluationTask,
    FailureCategory,
    FileDiff,
    ForgeRunEvidence,
    LeakageCheck,
    ProcessOutcome,
    RunCreateRequest,
    ShellOutput,
    WorkspaceCheck,
    WorkspaceObservation,
)
from app.runner import (
    DeterministicMockRunner,
    ForgeAgentRunner,
    continuity_db_diagnostic,
    create_runner,
)


def test_run_create_request_rejects_unknown_provider() -> None:
    with pytest.raises(ValidationError):
        RunCreateRequest(provider="unknown-provider", model="model-a")


def test_task_and_trace_accept_required_workspace_evidence_contracts() -> None:
    from datetime import UTC, datetime

    task = EvaluationTask(
        id="evidence-contract",
        title="Evidence contract",
        prompt="Run the task.",
        required_scores=["scope_ok"],
    )
    observation = WorkspaceObservation(
        available=True,
        source="filesystem_snapshot",
        changed_files=["src/app.py"],
    )
    now = datetime.now(UTC)
    trace = AgentTrace(
        task_id=task.id,
        user_prompt=task.prompt,
        model="model-a",
        provider=EvalProvider.MOCK,
        required_scores=task.required_scores,
        workspace_observation=observation,
        final_answer="done",
        started_at=now,
        ended_at=now,
        duration_ms=0,
    )

    assert trace.required_scores == ["scope_ok"]
    assert trace.workspace_observation == observation
    assert ProcessOutcome.CANCELLED.value == "cancelled"


def test_agent_adapter_metadata_is_attached_to_trace() -> None:
    from app.agent_adapter import AgentAdapterSpec

    spec = AgentAdapterSpec(name="forge", version="local", command="forge_eval_agent")

    assert spec.model_dump() == {
        "name": "forge",
        "version": "local",
        "command": "forge_eval_agent",
        "supports_trajectory": True,
    }


def test_task_supports_regression_and_fix_validation_commands() -> None:
    task = EvaluationTask(
        id="split-tests",
        title="Split tests",
        prompt="Fix bug",
        pass_to_pass_commands=["pytest tests/test_existing.py"],
        fail_to_pass_commands=["pytest tests/test_bug.py"],
    )

    assert task.pass_to_pass_commands == ["pytest tests/test_existing.py"]
    assert task.fail_to_pass_commands == ["pytest tests/test_bug.py"]


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


def test_mock_runner_preserves_forge_run_evidence_from_case_metadata() -> None:
    task = EvaluationTask(
        id="gateway-read-only-owner",
        title="Gateway read-only owner",
        prompt="Evaluate gateway read-only diagnostics owner safety.",
        context_files=["src/runtime.rs"],
        verification_command="cargo test gateway",
        expected_success=True,
        metadata={
            "mock": {
                "changed_files": [],
                "forge_run_evidence": {
                    "session_id": "session-gateway-1",
                    "loop_task_id": "loop-gateway-readonly",
                    "prepared_context": {
                        "turn_prepared": {
                            "context_estimate": {
                                "sources": [{"kind": "user_input", "label": "prompt"}]
                            }
                        }
                    },
                    "changed_files": [],
                    "verification": {"command": "cargo test gateway", "passed": True},
                    "provider_usage": {"input_tokens": 120, "output_tokens": 24},
                    "failure_category": "none",
                    "gateway": {
                        "ownership_mode": "gateway_read_only_owner",
                        "gateway_can_own_session": True,
                        "owner_run": {
                            "mode": "read_only_diagnostics",
                            "human_approved": True,
                            "side_effects": [],
                            "lease_id": "lease-1",
                            "heartbeat_ms": 1000,
                            "timeout_ms": 30000,
                        },
                    },
                },
            }
        },
    )
    runner = DeterministicMockRunner(provider="mock", model="deterministic-agent-v1")

    trace = runner.run_task(task)

    assert trace.forge_run_evidence == ForgeRunEvidence(
        session_id="session-gateway-1",
        loop_task_id="loop-gateway-readonly",
        prepared_context={
            "turn_prepared": {
                "context_estimate": {"sources": [{"kind": "user_input", "label": "prompt"}]}
            }
        },
        changed_files=[],
        verification={"command": "cargo test gateway", "passed": True},
        provider_usage={"input_tokens": 120, "output_tokens": 24},
        failure_category="none",
        gateway={
            "ownership_mode": "gateway_read_only_owner",
            "gateway_can_own_session": True,
            "owner_run": {
                "mode": "read_only_diagnostics",
                "human_approved": True,
                "side_effects": [],
                "lease_id": "lease-1",
                "heartbeat_ms": 1000,
                "timeout_ms": 30000,
            },
        },
    )


def test_forge_runner_reports_regression_validation_failure(tmp_path: Path) -> None:
    script = tmp_path / "fake_success.py"
    script.write_text(
        """
import json
import sys

json.dump({"changed_files": ["src/app.py"], "final_answer": "Done."}, sys.stdout)
""".strip(),
        encoding="utf-8",
    )
    regression_command = f'{sys.executable} -c "import sys; sys.exit(3)"'
    task = EvaluationTask(
        id="regression-split",
        title="Regression split",
        prompt="Fix bug.",
        expected_files_changed=["src/app.py"],
        pass_to_pass_commands=[regression_command],
        fail_to_pass_commands=[f"{sys.executable} -c \"print('bug fixed')\""],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.command == regression_command
    assert trace.failure_category == FailureCategory.VERIFICATION_FAILED
    assert trace.failure_reason == "Regression validation failed"


def test_forge_runner_reports_bugfix_validation_failure(tmp_path: Path) -> None:
    script = tmp_path / "fake_success.py"
    script.write_text(
        """
import json
import sys

json.dump({"changed_files": ["src/app.py"], "final_answer": "Done."}, sys.stdout)
""".strip(),
        encoding="utf-8",
    )
    regression_command = f"{sys.executable} -c \"print('existing tests pass')\""
    bugfix_command = f'{sys.executable} -c "import sys; sys.exit(4)"'
    task = EvaluationTask(
        id="bugfix-split",
        title="Bugfix split",
        prompt="Fix bug.",
        expected_files_changed=["src/app.py"],
        pass_to_pass_commands=[regression_command],
        fail_to_pass_commands=[bugfix_command],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.verification_result is not None
    assert trace.verification_result.command == bugfix_command
    assert [output.command for output in trace.shell_outputs[-2:]] == [
        regression_command,
        bugfix_command,
    ]
    assert trace.failure_category == FailureCategory.VERIFICATION_FAILED
    assert trace.failure_reason == "Bug-fix validation failed"


def test_sandbox_rejects_dirty_workspace_after_case(tmp_path: Path) -> None:
    from app.sandbox import assert_clean_workspace

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / ".env").write_text("SECRET=value\n", encoding="utf-8")

    result = assert_clean_workspace(workspace, allowed_untracked=[])

    assert result.ok is False
    assert ".env" in result.untracked_files


def test_sandbox_scrubs_future_state_git_history(tmp_path: Path) -> None:
    from app.sandbox import scrub_future_repo_state

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    run_git(workspace, "init", "-b", "main")
    run_git(workspace, "config", "user.email", "eval@example.com")
    run_git(workspace, "config", "user.name", "Eval Runner")
    (workspace / "app.txt").write_text("current\n", encoding="utf-8")
    run_git(workspace, "add", "app.txt")
    run_git(workspace, "commit", "-m", "current state")
    run_git(workspace, "checkout", "-b", "future-fix")
    (workspace / "app.txt").write_text("future fix\n", encoding="utf-8")
    run_git(workspace, "commit", "-am", "future fix")
    run_git(workspace, "checkout", "main")

    before = git_stdout(workspace, "log", "--all", "--oneline")
    assert "future fix" in before

    result = scrub_future_repo_state(workspace)

    after = git_stdout(workspace, "log", "--all", "--oneline")
    assert result.ok is True
    assert "future fix" not in after
    assert "branch:future-fix" in result.scrubbed_items


def test_sandbox_detects_future_state_lookup_commands() -> None:
    from datetime import UTC, datetime

    from app.sandbox import detect_future_state_lookup

    now = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    trace = AgentTrace(
        task_id="future-state-probe",
        user_prompt="Solve without peeking.",
        model="deterministic-agent-v1",
        provider="mock",
        tool_calls=[ShellOutput(command="git log --all --oneline")],
        final_answer="done",
        started_at=now,
        ended_at=now,
        duration_ms=10,
    )

    result = detect_future_state_lookup(trace)

    assert result.ok is False
    assert "git log --all" in result.findings[0]


def test_patch_replay_applies_trace_diff(tmp_path: Path) -> None:
    from app.patches import replay_patch

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "hello.txt").write_text("hello\n", encoding="utf-8")
    diff = FileDiff(
        path="hello.txt",
        change_type="modified",
        diff=(
            "diff --git a/hello.txt b/hello.txt\n"
            "--- a/hello.txt\n"
            "+++ b/hello.txt\n"
            "@@ -1 +1 @@\n"
            "-hello\n"
            "+hello forge\n"
        ),
    )

    result = replay_patch(workspace, [diff])

    assert result.ok is True
    assert (workspace / "hello.txt").read_text(encoding="utf-8") == "hello forge\n"


def test_patch_replay_applies_added_and_deleted_files(tmp_path: Path) -> None:
    from app.patches import replay_patch

    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "delete.txt").write_text("delete me\n", encoding="utf-8")
    diffs = [
        FileDiff(
            path="add.txt",
            change_type="added",
            diff=(
                "diff --git a/add.txt b/add.txt\n"
                "new file mode 100644\n"
                "--- /dev/null\n"
                "+++ b/add.txt\n"
                "@@ -0,0 +1 @@\n"
                "+added\n"
            ),
        ),
        FileDiff(
            path="delete.txt",
            change_type="deleted",
            diff=(
                "diff --git a/delete.txt b/delete.txt\n"
                "deleted file mode 100644\n"
                "--- a/delete.txt\n"
                "+++ /dev/null\n"
                "@@ -1 +0,0 @@\n"
                "-delete me\n"
            ),
        ),
    ]

    result = replay_patch(workspace, diffs)

    assert result.ok is True
    assert (workspace / "add.txt").read_text(encoding="utf-8") == "added\n"
    assert not (workspace / "delete.txt").exists()


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


def run_git(workspace: Path, *args: str) -> None:
    subprocess.run(
        ["git", *args],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=True,
    )


def git_stdout(workspace: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=True,
    )
    return completed.stdout


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


def test_runner_factory_rejects_unknown_provider() -> None:
    with pytest.raises(ValueError, match="Unsupported eval provider: unknown-provider"):
        create_runner(provider="unknown-provider", model="model-a")


def test_validate_execution_identity_is_fail_closed() -> None:
    from app.runner import validate_execution_identity

    assert validate_execution_identity(
        EvalProvider.FORGE,
        "model-a",
        "/cases/release",
    ) == (EvalProvider.FORGE, "model-a", "/cases/release")
    with pytest.raises(ValueError, match="missing provider"):
        validate_execution_identity(None, "model-a", "/cases/release")
    with pytest.raises(ValueError, match="missing model"):
        validate_execution_identity(EvalProvider.FORGE, " ", "/cases/release")
    with pytest.raises(ValueError, match="missing case_source"):
        validate_execution_identity(EvalProvider.FORGE, "model-a", "")


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
(workspace / "src" / "app.py").write_text("print('forge')\\n")
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
                "diff": (
                    "diff --git a/src/app.py b/src/app.py\\n"
                    "--- a/src/app.py\\n"
                    "+++ b/src/app.py\\n"
                    "@@ -1 +1 @@\\n"
                    "-print('hello')\\n"
                    "+print('forge')\\n"
                ),
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
        "forge_run_evidence": {
            "session_id": "session-1",
            "loop_task_id": "loop-1",
            "prompt": payload["prompt"],
            "normalized_goal": "modify-and-verify",
            "prepared_context": {
                "turn_prepared": {
                    "context_estimate": {
                        "sources": [{"kind": "user_input", "label": "prompt"}]
                    }
                }
            },
            "memory_audit": {"selected_memory_ids": []},
            "permission_decisions": [{"approved": True, "decision": "allow"}],
            "changed_files": ["src/app.py"],
            "verification": {"command": "pytest", "passed": True},
            "provider_usage": {"input_tokens": 120, "output_tokens": 40},
            "failure_category": "none",
        },
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
    assert trace.forge_run_evidence is not None
    assert trace.forge_run_evidence.session_id == "session-1"
    assert trace.forge_run_evidence.loop_task_id == "loop-1"
    assert trace.forge_run_evidence.normalized_goal == "modify-and-verify"


def test_forge_runner_detects_unreported_forbidden_workspace_change(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    (fixture / "allowed.txt").write_text("before\n", encoding="utf-8")
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,pathlib,sys\n"
        "payload=json.loads(sys.stdin.read())\n"
        "workspace=pathlib.Path(payload['workspace_path'])\n"
        "(workspace/'.env').write_text('SECRET=x\\n')\n"
        "json.dump({'changed_files': [], 'file_diffs': [], 'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="independent-scope",
        title="Independent scope",
        prompt="Change allowed.txt only.",
        fixture_path=str(fixture),
        expected_files_changed=["allowed.txt"],
        forbidden_files_changed=[".env"],
    )

    trace = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.changed_files == [".env"]
    assert trace.workspace_observation is not None
    assert trace.workspace_observation.reported_changed_files == []
    assert trace.scope_violations == ["forbidden_change:.env", "unexpected_change:.env"]


def test_forge_runner_does_not_attribute_setup_or_validation_changes_to_agent(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,sys\njson.dump({'changed_files': [], 'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="snapshot-boundaries",
        title="Snapshot boundaries",
        prompt="Run without edits.",
        fixture_path=str(fixture),
        setup_commands=[f"{sys.executable} -c \"open('setup.txt','w').write('x')\""],
        validation_commands=[f"{sys.executable} -c \"open('validation.txt','w').write('x')\""],
    )

    trace = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.changed_files == []


def test_forge_runner_isolates_forbidden_runtime_state_from_workspace(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("before\n", encoding="utf-8")
    script = tmp_path / "isolated_runtime_agent.py"
    script.write_text(
        "import json,pathlib,sqlite3,sys\n"
        "payload=json.loads(sys.stdin.read())\n"
        "workspace=pathlib.Path(payload['workspace_path'])\n"
        "runtime_state=pathlib.Path(payload['runtime_state_path'])\n"
        "continuity_db=pathlib.Path(payload['continuity_database_path'])\n"
        "assert runtime_state.parent == workspace.parent\n"
        "assert runtime_state != workspace and workspace not in runtime_state.parents\n"
        "assert continuity_db == runtime_state/'continuity.db'\n"
        "runtime_state.mkdir(parents=True,exist_ok=True)\n"
        "sqlite3.connect(runtime_state/'registry.db').close()\n"
        "sqlite3.connect(continuity_db).close()\n"
        "(workspace/'src'/'app.py').write_text('after\\n')\n"
        "json.dump({'final_answer':'done','changed_files':['src/app.py'],'file_diffs':["
        "{'path':'src/app.py','change_type':'modified','diff':"
        "\"diff --git a/src/app.py b/src/app.py\\n--- a/src/app.py\\n+++ b/src/app.py\\n@@ -1 +1 @@\\n-before\\n+after\\n\"}]},sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="isolated-runtime-state",
        title="Isolate runtime state",
        prompt="Change src/app.py only.",
        fixture_path=str(fixture),
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[
            ".forge/continuity.db",
            ".forge/registry.db",
        ],
    )

    trace = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.changed_files == ["src/app.py"]
    assert trace.scope_violations == []
    assert trace.patch_replay is not None
    assert trace.patch_replay.ok is True


def test_forge_runner_times_out_setup_command(tmp_path: Path) -> None:
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,sys\njson.dump({'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="setup-timeout",
        title="Setup timeout",
        prompt="Run.",
        setup_commands=[f'{sys.executable} -c "import time; time.sleep(10)"'],
    )

    trace = ForgeAgentRunner(
        command=[sys.executable, str(script)],
        setup_timeout_seconds=0.1,
    ).run_task(task)

    assert trace.error == "timeout"
    assert trace.failure_category == FailureCategory.TIMEOUT
    assert trace.shell_outputs[-1].exit_code == 124


def test_forge_runner_times_out_validation_command(tmp_path: Path) -> None:
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,sys\njson.dump({'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="validation-timeout",
        title="Validation timeout",
        prompt="Run.",
        validation_commands=[f'{sys.executable} -c "import time; time.sleep(10)"'],
    )

    trace = ForgeAgentRunner(
        command=[sys.executable, str(script)],
        validation_timeout_seconds=0.1,
    ).run_task(task)

    assert trace.error == "timeout"
    assert trace.failure_category == FailureCategory.TIMEOUT
    assert trace.verification_result is not None
    assert trace.verification_result.exit_code == 124


def test_forge_runner_cancellation_preserves_partial_output(tmp_path: Path) -> None:
    script = tmp_path / "agent.py"
    script.write_text(
        "import time\nprint('started', flush=True)\ntime.sleep(10)\n",
        encoding="utf-8",
    )
    cancelled = threading.Event()
    timer = threading.Timer(0.2, cancelled.set)
    timer.start()
    try:
        trace = ForgeAgentRunner(
            command=[sys.executable, "-u", str(script)],
        ).run_task(
            EvaluationTask(id="cancelled", title="Cancelled", prompt="Run."),
            cancel_requested=cancelled.is_set,
        )
    finally:
        timer.cancel()

    assert trace.error == "cancelled"
    assert trace.shell_outputs[-1].exit_code == 130
    assert "started" in trace.shell_outputs[-1].stdout


def test_forge_runner_records_failed_sandbox_scrub(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,sys\njson.dump({'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    failed = LeakageCheck(ok=False, findings=["git scrub timed_out"])
    monkeypatch.setattr("app.runner.scrub_future_repo_state", lambda *args, **kwargs: failed)

    trace = ForgeAgentRunner(command=[sys.executable, str(script)]).run_task(
        EvaluationTask(id="scrub-failed", title="Scrub failed", prompt="Run.")
    )

    assert trace.error == "sandbox_scrub_failed"
    assert trace.sandbox_scrub == failed


def test_forge_runner_records_failed_patch_replay(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    (fixture / "file.txt").write_text("before\n", encoding="utf-8")
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,pathlib,sys\n"
        "payload=json.loads(sys.stdin.read())\n"
        "path=pathlib.Path(payload['workspace_path'])/'file.txt'\n"
        "path.write_text('after\\n')\n"
        "json.dump({'final_answer':'done','file_diffs':["
        "{'path':'file.txt','change_type':'modified','diff':'invalid'}]},sys.stdout)\n",
        encoding="utf-8",
    )
    failed = WorkspaceCheck(ok=False, message="patch replay failed")
    monkeypatch.setattr("app.runner.replay_patch", lambda *args, **kwargs: failed)

    trace = ForgeAgentRunner(command=[sys.executable, str(script)]).run_task(
        EvaluationTask(
            id="patch-failed",
            title="Patch failed",
            prompt="Edit.",
            fixture_path=str(fixture),
        )
    )

    assert trace.changed_files == ["file.txt"]
    assert trace.patch_replay == failed
    assert trace.raw_events[-1]["event_type"] == "eval_patch_replay"


def test_forge_runner_reports_scope_violations_from_changed_files(tmp_path: Path) -> None:
    script = tmp_path / "fake_scope_violation.py"
    script.write_text(
        """
import json
import pathlib
import sys

payload = json.loads(sys.stdin.read())
workspace = pathlib.Path(payload["workspace_path"])
(workspace / "src").mkdir()
(workspace / "src" / "app.py").write_text("changed\\n")
(workspace / ".env").write_text("SECRET=x\\n")
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
conn.execute("CREATE TABLE continuity_experiences (id TEXT PRIMARY KEY, status TEXT, kind TEXT)")
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
conn.execute(
    "INSERT INTO continuity_experiences (id, status, kind) VALUES (?, ?, ?)",
    ("experience-1", "candidate", "workflow"),
)
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
        f'{sys.executable} -c "from pathlib import Path; '
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
    assert diagnostic["experience_status_counts"] == {"candidate": 1}
    assert diagnostic["experience_kind_counts"] == {"workflow": 1}
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


def test_continuity_db_diagnostic_handles_legacy_experience_schema(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    (workspace / ".forge").mkdir(parents=True)
    conn = sqlite3.connect(workspace / ".forge" / "continuity.db")
    conn.execute("CREATE TABLE continuity_experiences (id TEXT PRIMARY KEY)")
    conn.execute("INSERT INTO continuity_experiences (id) VALUES ('legacy-1')")
    conn.commit()
    conn.close()

    diagnostic = continuity_db_diagnostic(workspace)

    assert diagnostic is not None
    assert diagnostic["experience_count"] == 1
    assert diagnostic["experience_status_counts"] == {}
    assert diagnostic["experience_kind_counts"] == {}


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


def test_forge_runner_normalizes_budget_exhausted_failure_category(tmp_path: Path) -> None:
    script = tmp_path / "fake_budget_exhausted.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": ["src/app.py"],
        "error": "max_model_rounds_exceeded",
        "failure_category": "budget_exhausted",
        "failure_reason": "Forge headless eval exceeded the configured max model rounds.",
        "final_answer": "",
        "model_rounds": 10,
        "repair_attempts_used": 1,
        "validation_attempts": 2,
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="budget-exhausted",
        title="Budget exhausted",
        prompt="Run Forge.",
        expected_files_changed=["src/app.py"],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "max_model_rounds_exceeded"
    assert trace.failure_category == FailureCategory.BUDGET_EXHAUSTED
    assert trace.failure_reason == "Forge headless eval exceeded the configured max model rounds."
    assert trace.model_rounds == 10
    assert trace.repair_attempts_used == 1
    assert trace.validation_attempts == 2


def test_forge_runner_maps_timeout_field_from_max_duration_seconds(tmp_path: Path) -> None:
    script = tmp_path / "fake_timeout_field.py"
    script.write_text(
        """
import json
import sys

payload = json.loads(sys.stdin.read())
task = payload.get("task", {})
# Assert that runner mapped max_duration_seconds -> timeout_secs
assert task.get("timeout_secs") == 42
assert "max_duration_seconds" not in task
assert task.get("max_model_rounds") == 5
json.dump({"changed_files": ["src/app.py"], "final_answer": "Done."}, sys.stdout)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="timeout-field-mapping",
        title="Timeout field mapping",
        prompt="Run Forge.",
        expected_files_changed=["src/app.py"],
        max_duration_seconds=42,
        max_model_rounds=5,
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error is None
    assert trace.failure_category == FailureCategory.NONE


def test_forge_runner_accepts_json_object_after_log_lines(tmp_path: Path) -> None:
    script = tmp_path / "fake_forge_with_logs.py"
    script.write_text(
        """
import json

print("starting forge eval")
print(json.dumps({
    "final_answer": "done",
    "verification_result": {"command": "pytest", "passed": True, "exit_code": 0},
    "changed_files": ["src/calculator.py"],
    "file_diffs": [],
    "tool_calls": [],
    "shell_outputs": []
}))
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="small-edit-success",
        title="Small edit",
        prompt="Fix add",
        expected_files_changed=["src/calculator.py"],
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.failure_category == FailureCategory.NONE
    assert trace.final_answer == "done"


def test_forge_runner_reports_missing_final_answer_as_contract_error(
    tmp_path: Path,
) -> None:
    script = tmp_path / "fake_missing_final_answer.py"
    script.write_text(
        """
import json
import sys

json.dump({"changed_files": []}, sys.stdout)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="missing-final-answer",
        title="Missing final answer",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "invalid_forge_trace"
    assert trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR
    assert "final_answer" in (trace.failure_reason or "")


def test_forge_runner_reports_malformed_tool_calls_as_contract_error(
    tmp_path: Path,
) -> None:
    script = tmp_path / "fake_malformed_tool_calls.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": [],
        "final_answer": "Done.",
        "tool_calls": {"command": "not-a-list"},
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="malformed-tool-calls",
        title="Malformed tool calls",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "invalid_forge_trace"
    assert trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR
    assert "ValidationError" in (trace.failure_reason or "")


def test_forge_runner_maps_unknown_failure_category_to_contract_error(
    tmp_path: Path,
) -> None:
    script = tmp_path / "fake_unknown_failure_category.py"
    script.write_text(
        """
import json
import sys

json.dump(
    {
        "changed_files": [],
        "final_answer": "Done.",
        "failure_category": "mystery_failure",
    },
    sys.stdout,
)
""".strip(),
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="unknown-failure-category",
        title="Unknown failure category",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "forge_contract_error"
    assert trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR


def test_forge_runner_reports_invalid_stdout_as_contract_error(tmp_path: Path) -> None:
    script = tmp_path / "fake_invalid_stdout.py"
    script.write_text(
        "import sys\nprint('not json')\nprint('bad stderr', file=sys.stderr)\n",
        encoding="utf-8",
    )
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
    assert "JSONDecodeError" in (trace.failure_reason or "")
    assert "stdout preview: not json" in (trace.failure_reason or "")
    assert "stderr preview: bad stderr" in (trace.failure_reason or "")
    assert trace.shell_outputs[-1].stdout == "not json\n"
    assert trace.shell_outputs[-1].stderr == "bad stderr\n"


def test_forge_runner_reports_log_lines_without_json_as_contract_error(
    tmp_path: Path,
) -> None:
    script = tmp_path / "fake_logs_without_json.py"
    script.write_text(
        "import sys\nprint('starting forge eval')\nprint('still not json', file=sys.stderr)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="logs-without-json",
        title="Logs without json",
        prompt="Run Forge.",
    )

    trace = ForgeAgentRunner(
        provider="forge",
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)

    assert trace.error == "invalid_forge_trace"
    assert trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR
    assert "stdout preview: starting forge eval" in (trace.failure_reason or "")
    assert "stderr preview: still not json" in (trace.failure_reason or "")


def test_forge_runner_returns_runner_error_when_command_is_missing() -> None:
    task = EvaluationTask(id="missing-command", title="Missing command", prompt="Run Forge.")

    trace = ForgeAgentRunner(provider="forge", model="local-forge", command=None).run_task(task)

    assert trace.error == "forge_command_not_configured"
    assert trace.failure_category == FailureCategory.RUNNER_ERROR
    assert trace.failure_reason == "Set FORGE_EVAL_FORGE_AGENT_COMMAND to run provider=forge."
