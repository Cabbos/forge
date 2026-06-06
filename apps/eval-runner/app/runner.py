import json
import shlex
import shutil
import sqlite3
import subprocess
import tempfile
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Protocol

from pydantic import TypeAdapter, ValidationError

from app.models import (
    AgentTrace,
    EvaluationTask,
    FailureCategory,
    FileDiff,
    ShellOutput,
    VerificationResult,
)
from app.trace import duration_ms, utc_now

DEFAULT_FORGE_TIMEOUT_SECONDS = 900


class EvalRunner(Protocol):
    def run_task(self, task: EvaluationTask) -> AgentTrace: ...


class DeterministicMockRunner:
    """A deterministic stand-in for a real coding agent execution loop."""

    def __init__(self, provider: str = "mock", model: str = "deterministic-agent-v1") -> None:
        self.provider = provider
        self.model = model

    def run_task(self, task: EvaluationTask) -> AgentTrace:
        started_at = utc_now()
        target_file = task.context_files[0] if task.context_files else "workspace/changes.patch"
        mock = mock_metadata(task)
        changed_files = mock_changed_files(mock, default_target_file=target_file)
        file_diffs = build_mock_file_diffs(changed_files)

        tool_calls = mock_tool_calls(
            mock,
            default_target_file=target_file,
            context_file_count=len(task.context_files),
        )

        shell_outputs: list[ShellOutput] = []
        verification_result: VerificationResult | None = None
        error: str | None = None
        failure_reason: str | None = None
        failure_category = FailureCategory.NONE
        simulated_error = mock.get("error")
        if simulated_error is not None:
            error = str(simulated_error)
            failure_reason = str(mock.get("failure_reason") or "Mock runner simulated a failure.")
            failure_category = FailureCategory(
                mock.get("failure_category", FailureCategory.RUNNER_ERROR)
            )
            ended_at = utc_now()
            return AgentTrace(
                task_id=task.id,
                user_prompt=task.prompt,
                model=self.model,
                provider=self.provider,
                context_files=task.context_files,
                raw_events=list(mock.get("raw_events", [])),
                tool_calls=tool_calls,
                shell_outputs=shell_outputs,
                file_diffs=file_diffs,
                changed_files=changed_files,
                final_answer=f"Mock agent simulated failure for task {task.id}.",
                verification_result=None,
                error=error,
                failure_reason=failure_reason,
                failure_category=failure_category,
                model_rounds=int(mock.get("model_rounds", 0)),
                confirm_requests=int(mock.get("confirm_requests", 0)),
                input_tokens=mock.get("input_tokens"),
                output_tokens=mock.get("output_tokens"),
                started_at=started_at,
                ended_at=ended_at,
                duration_ms=int(mock.get("duration_ms", duration_ms(started_at, ended_at))),
            )

        if task.verification_command is None:
            error = "no_verification"
            failure_reason = "Task does not define a verification command."
            failure_category = FailureCategory.NO_VERIFICATION
        else:
            exit_code = 0 if task.expected_success else 1
            stdout = (
                "All verification checks passed." if task.expected_success else "1 test failed."
            )
            stderr = "" if task.expected_success else "AssertionError: simulated failure"
            shell_outputs.append(
                ShellOutput(
                    command=task.verification_command,
                    stdout=stdout,
                    stderr=stderr,
                    exit_code=exit_code,
                    duration_ms=120,
                )
            )
            verification_result = VerificationResult(
                command=task.verification_command,
                passed=task.expected_success,
                stdout=stdout,
                stderr=stderr,
                exit_code=exit_code,
                duration_ms=120,
            )
            if not task.expected_success:
                error = "verification_failed"
                failure_reason = "Mock verification command returned a non-zero exit code."
                failure_category = FailureCategory.VERIFICATION_FAILED

        scope_violations = scope_violations_for(task, changed_files)
        if scope_violations and failure_category == FailureCategory.NONE:
            error = "scope_violation"
            failure_reason = "Changed files violated eval scope: " + ", ".join(scope_violations)
            failure_category = FailureCategory.SCOPE_VIOLATION

        ended_at = utc_now()
        final_answer = (
            f"Mock agent completed task {task.id} with deterministic trace data."
            if error is None
            else f"Mock agent attempted task {task.id}, but evaluation did not pass."
        )

        return AgentTrace(
            task_id=task.id,
            user_prompt=task.prompt,
            model=self.model,
            provider=self.provider,
            context_files=task.context_files,
            raw_events=list(mock.get("raw_events", [])),
            tool_calls=tool_calls,
            shell_outputs=shell_outputs,
            file_diffs=file_diffs,
            changed_files=changed_files,
            scope_violations=scope_violations,
            final_answer=final_answer,
            verification_result=verification_result,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            model_rounds=int(mock.get("model_rounds", 0)),
            confirm_requests=int(mock.get("confirm_requests", 0)),
            input_tokens=mock.get("input_tokens"),
            output_tokens=mock.get("output_tokens"),
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=int(mock.get("duration_ms", duration_ms(started_at, ended_at))),
        )


class ForgeAgentRunner:
    """Adapter for running a real Forge agent through an external headless command.

    The command receives a JSON payload on stdin and must write a JSON object to stdout.
    This keeps the eval service independent from Forge's current Tauri process shape while
    preserving the trace contract used by the API.
    """

    def __init__(
        self,
        provider: str = "forge",
        model: str = "local-forge",
        command: str | Sequence[str] | None = None,
    ) -> None:
        self.provider = provider
        self.model = model
        self.command = normalize_command(command)

    def run_task(self, task: EvaluationTask) -> AgentTrace:
        started_at = utc_now()
        if self.command is None:
            return self._error_trace(
                task=task,
                started_at=started_at,
                error="forge_command_not_configured",
                failure_reason="Set FORGE_EVAL_FORGE_AGENT_COMMAND to run provider=forge.",
                failure_category=FailureCategory.RUNNER_ERROR,
            )

        timeout = task.max_duration_seconds or DEFAULT_FORGE_TIMEOUT_SECONDS
        with tempfile.TemporaryDirectory(prefix=f"forge-eval-{task.id}-") as temp_dir:
            workspace = prepare_workspace(task, Path(temp_dir))
            setup_outputs = run_setup_commands(task, workspace)
            setup_failure = next(
                (output for output in setup_outputs if output.exit_code != 0),
                None,
            )
            if setup_failure is not None:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="setup_failed",
                    failure_reason=f"Setup command failed: {setup_failure.command}",
                    failure_category=FailureCategory.RUNNER_ERROR,
                    shell_outputs=setup_outputs,
                )

            payload = {
                "task": task.model_dump(mode="json"),
                "prompt": task.prompt,
                "provider": self.provider,
                "model": self.model,
                "workspace_path": str(workspace),
            }
            try:
                completed = subprocess.run(
                    self.command,
                    input=json.dumps(payload),
                    text=True,
                    capture_output=True,
                    cwd=workspace,
                    timeout=timeout,
                    check=False,
                )
            except subprocess.TimeoutExpired as exc:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="timeout",
                    failure_reason=f"Forge command exceeded {timeout}s.",
                    failure_category=FailureCategory.TIMEOUT,
                    shell_outputs=[
                        ShellOutput(
                            command=command_label(self.command),
                            stdout=exc.stdout or "",
                            stderr=exc.stderr or "",
                            exit_code=124,
                            duration_ms=timeout * 1000,
                        )
                    ],
                )

            command_output = ShellOutput(
                command=command_label(self.command),
                stdout=completed.stdout,
                stderr=completed.stderr,
                exit_code=completed.returncode,
                duration_ms=duration_ms(started_at, utc_now()),
            )
            if completed.returncode != 0:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="forge_command_failed",
                    failure_reason=f"Forge command exited with {completed.returncode}.",
                    failure_category=FailureCategory.RUNNER_ERROR,
                    shell_outputs=[*setup_outputs, command_output],
                )

            try:
                raw_payload = json.loads(completed.stdout or "{}")
                trace = self._trace_from_payload(
                    task,
                    raw_payload,
                    started_at,
                    setup_outputs,
                    workspace,
                )
            except (json.JSONDecodeError, ValidationError, TypeError, ValueError) as exc:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="invalid_forge_trace",
                    failure_reason=f"Forge command returned invalid trace JSON: {exc}",
                    failure_category=FailureCategory.FORGE_CONTRACT_ERROR,
                    shell_outputs=[*setup_outputs, command_output],
                )

            return trace

    def _trace_from_payload(
        self,
        task: EvaluationTask,
        payload: dict[str, Any],
        started_at,
        setup_outputs: list[ShellOutput],
        workspace: Path,
    ) -> AgentTrace:
        ended_at = utc_now()
        tool_calls = TypeAdapter(list[ShellOutput]).validate_python(payload.get("tool_calls", []))
        validation_outputs = [
            *run_validation_commands(task, workspace),
            *run_post_validation_commands(task, workspace),
        ]
        shell_outputs = TypeAdapter(list[ShellOutput]).validate_python(
            [*setup_outputs, *payload.get("shell_outputs", []), *validation_outputs]
        )
        file_diffs = TypeAdapter(list[FileDiff]).validate_python(payload.get("file_diffs", []))
        verification_result = (
            VerificationResult.model_validate(payload["verification_result"])
            if payload.get("verification_result") is not None
            else None
        )
        if validation_outputs:
            last_validation = first_failed_output(validation_outputs) or validation_outputs[-1]
            verification_result = VerificationResult(
                command=last_validation.command,
                passed=last_validation.exit_code == 0,
                stdout=last_validation.stdout,
                stderr=last_validation.stderr,
                exit_code=last_validation.exit_code,
                duration_ms=last_validation.duration_ms,
            )
        changed_files = list(payload.get("changed_files") or [diff.path for diff in file_diffs])
        raw_events = list(payload.get("raw_events", []))
        headless_continuity_diagnostic = headless_continuity_diagnostic_from_payload(payload)
        if headless_continuity_diagnostic is not None:
            raw_events.append(headless_continuity_diagnostic)
        continuity_diagnostic = continuity_db_diagnostic(workspace)
        if continuity_diagnostic is not None:
            raw_events.append(continuity_diagnostic)
        scope_violations = scope_violations_for(task, changed_files)
        failure_category = normalize_failure_category(
            payload.get("failure_category", FailureCategory.NONE)
        )
        error = normalize_error(payload.get("error"), failure_category)
        failure_reason = payload.get("failure_reason")

        if (
            validation_outputs
            and verification_result is not None
            and verification_result.passed
            and failure_category == FailureCategory.VERIFICATION_FAILED
        ):
            error = None
            failure_reason = None
            failure_category = FailureCategory.NONE

        if (
            verification_result is not None
            and not verification_result.passed
            and failure_category == FailureCategory.NONE
        ):
            error = "verification_failed"
            failure_reason = f"Validation command failed: {verification_result.command}"
            failure_category = FailureCategory.VERIFICATION_FAILED

        if scope_violations and failure_category == FailureCategory.NONE:
            error = "scope_violation"
            failure_reason = "Changed files violated eval scope: " + ", ".join(scope_violations)
            failure_category = FailureCategory.SCOPE_VIOLATION

        return AgentTrace(
            task_id=task.id,
            user_prompt=task.prompt,
            model=self.model,
            provider=self.provider,
            context_files=task.context_files,
            raw_events=raw_events,
            tool_calls=tool_calls,
            shell_outputs=shell_outputs,
            file_diffs=file_diffs,
            changed_files=changed_files,
            scope_violations=scope_violations,
            final_answer=payload.get("final_answer", ""),
            verification_result=verification_result,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            model_rounds=payload.get("model_rounds", 0),
            confirm_requests=payload.get("confirm_requests", 0),
            input_tokens=payload.get("input_tokens"),
            output_tokens=payload.get("output_tokens"),
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )

    def _error_trace(
        self,
        *,
        task: EvaluationTask,
        started_at,
        error: str,
        failure_reason: str,
        failure_category: FailureCategory,
        shell_outputs: list[ShellOutput] | None = None,
    ) -> AgentTrace:
        ended_at = utc_now()
        return AgentTrace(
            task_id=task.id,
            user_prompt=task.prompt,
            model=self.model,
            provider=self.provider,
            context_files=task.context_files,
            tool_calls=[],
            shell_outputs=shell_outputs or [],
            file_diffs=[],
            changed_files=[],
            final_answer="Forge runner could not complete the task.",
            verification_result=None,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )


def create_runner(
    provider: str,
    model: str,
    forge_command: str | Sequence[str] | None = None,
) -> EvalRunner:
    if provider == "forge":
        return ForgeAgentRunner(provider=provider, model=model, command=forge_command)
    return DeterministicMockRunner(provider=provider, model=model)


def normalize_command(command: str | Sequence[str] | None) -> list[str] | None:
    if command is None:
        return None
    if isinstance(command, str):
        return shlex.split(command)
    return [str(part) for part in command]


def command_label(command: Sequence[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def mock_metadata(task: EvaluationTask) -> dict[str, Any]:
    raw_mock = task.metadata.get("mock", {})
    return raw_mock if isinstance(raw_mock, dict) else {}


def mock_changed_files(mock: dict[str, Any], *, default_target_file: str) -> list[str]:
    raw_changed_files = mock.get("changed_files")
    if isinstance(raw_changed_files, list):
        return [str(path) for path in raw_changed_files]
    return [default_target_file]


def mock_tool_calls(
    mock: dict[str, Any],
    *,
    default_target_file: str,
    context_file_count: int,
) -> list[ShellOutput]:
    raw_tool_calls = mock.get("tool_calls")
    if isinstance(raw_tool_calls, list):
        return TypeAdapter(list[ShellOutput]).validate_python(raw_tool_calls)

    raw_tool_commands = mock.get("tool_commands")
    if isinstance(raw_tool_commands, list):
        return [
            ShellOutput(
                command=str(command),
                stdout="ok",
                stderr="",
                exit_code=0,
                duration_ms=25,
            )
            for command in raw_tool_commands
        ]

    return [
        ShellOutput(
            command="read_context",
            stdout=f"Loaded {context_file_count} context file(s).",
            stderr="",
            exit_code=0,
            duration_ms=25,
        ),
        ShellOutput(
            command="edit_files",
            stdout=f"Prepared deterministic patch for {default_target_file}.",
            stderr="",
            exit_code=0,
            duration_ms=35,
        ),
    ]


def build_mock_file_diffs(changed_files: Sequence[str]) -> list[FileDiff]:
    return [
        FileDiff(
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
        for path in changed_files
    ]


def prepare_workspace(task: EvaluationTask, temp_dir: Path) -> Path:
    workspace = temp_dir / "workspace"
    if task.fixture_path is None:
        workspace.mkdir(parents=True, exist_ok=True)
        return workspace

    fixture = Path(task.fixture_path).expanduser()
    if not fixture.exists():
        workspace.mkdir(parents=True, exist_ok=True)
        return workspace

    shutil.copytree(fixture, workspace)
    return workspace


def continuity_db_diagnostic(workspace: Path) -> dict[str, Any] | None:
    db_path = workspace / ".forge" / "continuity.db"
    if not db_path.exists():
        return None

    diagnostic: dict[str, Any] = {
        "event_type": "eval_continuity_db_diagnostic",
        "exists": True,
        "db_path": ".forge/continuity.db",
    }
    try:
        conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        try:
            diagnostic["event_counts"] = continuity_event_counts(conn)
            diagnostic["experience_count"] = count_table_rows(
                conn, "continuity_experiences"
            )
            diagnostic["fts_count"] = count_table_rows(
                conn, "continuity_experiences_fts"
            )
            diagnostic["formed_reflection_count"] = count_table_rows(
                conn, "continuity_formed_reflections"
            )
            diagnostic["reflection_episodes"] = continuity_reflection_episodes(conn)
        finally:
            conn.close()
    except sqlite3.Error as exc:
        diagnostic["error"] = str(exc)
    return diagnostic


def headless_continuity_diagnostic_from_payload(payload: dict[str, Any]) -> dict[str, Any] | None:
    formed_count = payload.get("headless_continuity_formed_count")
    error = payload.get("headless_continuity_error")
    if formed_count is None and error is None:
        return None
    return {
        "event_type": "eval_headless_continuity_diagnostic",
        "formed_count": formed_count,
        "error": error,
    }


def continuity_event_counts(conn: sqlite3.Connection) -> dict[str, int]:
    if not sqlite_table_exists(conn, "continuity_events"):
        return {}
    rows = conn.execute(
        "SELECT event_type, COUNT(*) FROM continuity_events GROUP BY event_type"
    ).fetchall()
    return {str(event_type): int(count) for event_type, count in rows}


def continuity_reflection_episodes(conn: sqlite3.Connection) -> list[dict[str, Any]]:
    if not sqlite_table_exists(conn, "continuity_events"):
        return []
    rows = conn.execute(
        "SELECT event_json FROM continuity_events "
        "WHERE event_type = 'reflection' LIMIT 5"
    ).fetchall()
    episodes: list[dict[str, Any]] = []
    for (event_json,) in rows:
        try:
            payload = json.loads(event_json)
        except (TypeError, json.JSONDecodeError):
            continue
        reflection = payload.get("reflection") if isinstance(payload, dict) else None
        if not isinstance(reflection, dict):
            continue
        episode = reflection.get("episode")
        if not isinstance(episode, dict):
            episodes.append({"has_episode": False})
            continue
        episodes.append(
            {
                "user_goal_summary": episode.get("user_goal_summary"),
                "changed_files": list(episode.get("changed_files") or []),
                "file_changes_count": len(episode.get("file_changes") or []),
                "file_changes": [
                    {
                        "path": change.get("path"),
                        "operation": change.get("operation"),
                        "tool_name": change.get("tool_name"),
                    }
                    for change in (episode.get("file_changes") or [])[:5]
                    if isinstance(change, dict)
                ],
                "tool_count": int(episode.get("tool_count") or 0),
                "failed_tools": int(episode.get("failed_tools") or 0),
                "notable_failures": [
                    {
                        "tool_name": failure.get("tool_name"),
                        "command": failure.get("command"),
                        "summary": text_preview(failure.get("summary"), 300),
                    }
                    for failure in (episode.get("notable_failures") or [])[:5]
                    if isinstance(failure, dict)
                ],
                "outcome": episode.get("outcome"),
                "verification_status": episode.get("verification_status"),
            }
        )
    return episodes


def text_preview(value: Any, limit: int) -> str | None:
    if value is None:
        return None
    text = str(value)
    if len(text) <= limit:
        return text
    return f"{text[:limit]}..."


def count_table_rows(conn: sqlite3.Connection, table: str) -> int | None:
    if not sqlite_table_exists(conn, table):
        return None
    return int(conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])


def sqlite_table_exists(conn: sqlite3.Connection, table: str) -> bool:
    row = conn.execute(
        "SELECT COUNT(1) FROM sqlite_master WHERE type = 'table' AND name = ?",
        (table,),
    ).fetchone()
    return bool(row and row[0])


def run_setup_commands(task: EvaluationTask, workspace: Path) -> list[ShellOutput]:
    return run_shell_commands(task.setup_commands, workspace)


def run_validation_commands(task: EvaluationTask, workspace: Path) -> list[ShellOutput]:
    return run_shell_commands(task.validation_commands, workspace)


def run_post_validation_commands(task: EvaluationTask, workspace: Path) -> list[ShellOutput]:
    return run_shell_commands(task.post_validation_commands, workspace)


def first_failed_output(outputs: Sequence[ShellOutput]) -> ShellOutput | None:
    return next((output for output in outputs if output.exit_code != 0), None)


def normalize_failure_category(value: str | FailureCategory) -> FailureCategory:
    if isinstance(value, FailureCategory):
        return value

    aliases = {
        "api": FailureCategory.RUNNER_ERROR,
        "agent_error": FailureCategory.RUNNER_ERROR,
        "missing_api_key": FailureCategory.RUNNER_ERROR,
        "setup_failed": FailureCategory.RUNNER_ERROR,
        "tool": FailureCategory.TOOL_ERROR,
        "tool_failed": FailureCategory.TOOL_ERROR,
        "verification": FailureCategory.VERIFICATION_FAILED,
    }
    normalized = aliases.get(str(value))
    if normalized is not None:
        return normalized

    try:
        return FailureCategory(str(value))
    except ValueError:
        return FailureCategory.FORGE_CONTRACT_ERROR


def normalize_error(error: str | None, failure_category: FailureCategory) -> str | None:
    if error == "verification" and failure_category == FailureCategory.VERIFICATION_FAILED:
        return "verification_failed"
    if error is None and failure_category != FailureCategory.NONE:
        return failure_category.value
    return error


def run_shell_commands(commands: Sequence[str], workspace: Path) -> list[ShellOutput]:
    outputs: list[ShellOutput] = []
    for command in commands:
        started_at = utc_now()
        completed = subprocess.run(
            command,
            shell=True,
            text=True,
            capture_output=True,
            cwd=workspace,
            check=False,
        )
        outputs.append(
            ShellOutput(
                command=command,
                stdout=completed.stdout,
                stderr=completed.stderr,
                exit_code=completed.returncode,
                duration_ms=duration_ms(started_at, utc_now()),
            )
        )
    return outputs


def scope_violations_for(task: EvaluationTask, changed_files: Sequence[str]) -> list[str]:
    expected = set(task.expected_files_changed)
    forbidden = set(task.forbidden_files_changed)
    violations: list[str] = []
    for path in changed_files:
        if path in forbidden:
            violations.append(f"forbidden_change:{path}")
        if expected and path not in expected:
            violations.append(f"unexpected_change:{path}")
    return violations
