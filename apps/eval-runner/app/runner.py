from app.models import (
    AgentTrace,
    EvaluationTask,
    FailureCategory,
    ShellOutput,
    VerificationResult,
)
from app.trace import build_mock_file_diff, duration_ms, utc_now


class DeterministicMockRunner:
    """A deterministic stand-in for a real coding agent execution loop."""

    def __init__(self, provider: str = "mock", model: str = "deterministic-agent-v1") -> None:
        self.provider = provider
        self.model = model

    def run_task(self, task: EvaluationTask) -> AgentTrace:
        started_at = utc_now()
        target_file = task.context_files[0] if task.context_files else "workspace/changes.patch"

        tool_calls = [
            ShellOutput(
                command="read_context",
                stdout=f"Loaded {len(task.context_files)} context file(s).",
                stderr="",
                exit_code=0,
                duration_ms=25,
            ),
            ShellOutput(
                command="edit_files",
                stdout=f"Prepared deterministic patch for {target_file}.",
                stderr="",
                exit_code=0,
                duration_ms=35,
            ),
        ]

        shell_outputs: list[ShellOutput] = []
        verification_result: VerificationResult | None = None
        error: str | None = None
        failure_reason: str | None = None
        failure_category = FailureCategory.NONE

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
            tool_calls=tool_calls,
            shell_outputs=shell_outputs,
            file_diffs=[build_mock_file_diff(task)],
            final_answer=final_answer,
            verification_result=verification_result,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )
