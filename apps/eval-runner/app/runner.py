import json
import os
import re
import shlex
import shutil
import sqlite3
import tempfile
from collections.abc import Mapping, Sequence
from datetime import datetime
from fnmatch import fnmatchcase
from pathlib import Path
from typing import Any, Literal, Protocol

from pydantic import TypeAdapter, ValidationError

from app.execution_identity import (
    ExecutionIdentityV1,
    validate_execution_identity_event,
)
from app.harness_contracts import (
    HarnessPolicyV1,
    build_context_manifest,
    build_task_execution_contract,
    context_manifest_fingerprint,
    harness_contract_evidence,
)
from app.hidden_verifiers import run_hidden_verifier, verifier_command
from app.models import (
    AgentTrace,
    EvalModel,
    EvalProvider,
    EvaluationTask,
    FailureCategory,
    FileDiff,
    ForgeRunEvidence,
    LeakageCheck,
    ProcessOutcome,
    ShellOutput,
    VerificationResult,
    WorkspaceCheck,
    WorkspaceObservation,
)
from app.patches import replay_patch
from app.process_control import CancelRequested, never_cancelled, run_bounded_process
from app.provider_usage import (
    estimate_provider_cost_usd,
    is_kimi_k3_usage,
    kimi_k3_pricing_snapshot,
)
from app.sandbox import scrub_future_repo_state
from app.trace import duration_ms, utc_now
from app.workspace_observer import (
    EPHEMERAL_CACHE_DIRS,
    observe_workspace_changes,
    snapshot_workspace,
)

DEFAULT_FORGE_TIMEOUT_SECONDS = 900
DEFAULT_SETUP_TIMEOUT_SECONDS = 300
DEFAULT_VALIDATION_TIMEOUT_SECONDS = 300
_ISOLATED_CHILD_ENV_KEYS = frozenset(
    {
        "HOME",
        "USERPROFILE",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "FORGE_EVAL_RUNTIME_STATE",
        "XDG_CACHE_HOME",
        "BUN_INSTALL_CACHE_DIR",
        "UV_CACHE_DIR",
        "UV_NO_CONFIG",
        "UV_PROJECT_ENVIRONMENT",
        "PYTHONDONTWRITEBYTECODE",
        "PYTHONPYCACHEPREFIX",
        "PATH",
        "FORGE_EVAL_REAL_UV",
    }
)


def _validate_child_env_overrides(
    overrides: Mapping[str, str] | None,
) -> dict[str, str]:
    if overrides is None:
        return {}
    normalized: dict[str, str] = {}
    for key, value in overrides.items():
        if not isinstance(key, str) or not key or "=" in key or "\x00" in key:
            raise ValueError("child environment override has an invalid variable name")
        if key in _ISOLATED_CHILD_ENV_KEYS:
            raise ValueError(f"child environment override cannot replace isolated variable {key}")
        if not isinstance(value, str) or "\x00" in value:
            raise ValueError("child environment override has an invalid variable value")
        normalized[key] = value
    return normalized


def _install_uv_scope_wrapper(runtime_state: Path, child_env: dict[str, str]) -> dict[str, str]:
    """Keep agent-side ``uv run`` commands from mutating the observed workspace.

    Forge cases provide explicit ``--no-project`` validation commands, but an agent
    may improvise ``uv run pytest`` while exploring.  uv then creates a project
    lockfile in the fixture.  A runtime-local shim injects ``--no-project`` for
    unqualified ``uv run`` calls; explicit project selection remains untouched.
    """

    real_uv = shutil.which("uv", path=child_env.get("PATH"))
    if real_uv is None:
        return child_env

    wrapper_dir = runtime_state / "bin"
    wrapper_dir.mkdir(parents=True, exist_ok=True)
    wrapper_path = wrapper_dir / "uv"
    wrapper_path.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        'if [ "${1:-}" = "run" ]; then\n'
        '  subcommand="$1"\n'
        "  shift\n"
        "  project_mode=0\n"
        '  for arg in "$@"; do\n'
        '    case "$arg" in\n'
        "      --no-project|--project|--project=*) project_mode=1 ;;\n"
        "    esac\n"
        "  done\n"
        '  if [ "$project_mode" -eq 0 ]; then\n'
        '    set -- "$subcommand" "--no-project" "$@"\n'
        "  else\n"
        '    set -- "$subcommand" "$@"\n'
        "  fi\n"
        "fi\n"
        'exec "$FORGE_EVAL_REAL_UV" "$@"\n',
        encoding="utf-8",
    )
    wrapper_path.chmod(0o755)
    wrapped_env = dict(child_env)
    wrapped_env["FORGE_EVAL_REAL_UV"] = real_uv
    wrapped_env["PATH"] = os.pathsep.join(
        part for part in (str(wrapper_dir), child_env.get("PATH", "")) if part
    )
    return wrapped_env


class ForgeCredentialPreflight(EvalModel):
    ok: bool
    provider: str
    model: str
    transport: str | None = None
    endpoint_family: str | None = None
    base_url: str | None = None
    provider_call_started: Literal[False]
    error: str | None = None


class EvalRunner(Protocol):
    def run_task(
        self,
        task: EvaluationTask,
        *,
        cancel_requested: CancelRequested = never_cancelled,
    ) -> AgentTrace: ...


class DeterministicMockRunner:
    """A deterministic stand-in for a real coding agent execution loop."""

    def __init__(
        self,
        provider: EvalProvider | str = EvalProvider.MOCK,
        model: str = "deterministic-agent-v1",
    ) -> None:
        self.provider = EvalProvider(provider)
        self.model = model

    def run_task(
        self,
        task: EvaluationTask,
        *,
        cancel_requested: CancelRequested = never_cancelled,
    ) -> AgentTrace:
        started_at = utc_now()
        if cancel_requested():
            ended_at = utc_now()
            return AgentTrace(
                task_id=task.id,
                user_prompt=task.prompt,
                model=self.model,
                provider=self.provider,
                required_scores=task.required_scores,
                changed_files=[],
                workspace_observation=WorkspaceObservation(
                    available=True,
                    source="deterministic_mock_contract",
                    changed_files=[],
                    reported_changed_files=[],
                ),
                final_answer="Mock agent was cancelled before task execution.",
                error="cancelled",
                failure_reason="Eval run was cancelled before task execution.",
                failure_category=FailureCategory.RUNNER_ERROR,
                started_at=started_at,
                ended_at=ended_at,
                duration_ms=duration_ms(started_at, ended_at),
            )
        # Prefer the task's declared output path when context is intentionally
        # empty.  ForgeBench cases keep controller-owned scope in
        # ``expected_files_changed`` rather than exposing context files; using
        # the synthetic fallback in that shape would make the golden mock
        # manufacture its own out-of-scope change and poison the trust gate.
        target_file = (
            task.expected_files_changed[0]
            if task.expected_files_changed
            else (task.context_files[0] if task.context_files else "workspace/changes.patch")
        )
        mock = mock_metadata(task)
        forge_run_evidence = mock_forge_run_evidence(mock)
        if task.metadata.get("expected_no_change") is True and "changed_files" not in mock:
            changed_files = []
        else:
            changed_files = mock_changed_files(mock, default_target_file=target_file)
        workspace_observation = WorkspaceObservation(
            available=True,
            source="deterministic_mock_contract",
            changed_files=changed_files,
            reported_changed_files=changed_files,
        )
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
                required_scores=task.required_scores,
                context_files=task.context_files,
                raw_events=list(mock.get("raw_events", [])),
                tool_calls=tool_calls,
                shell_outputs=shell_outputs,
                file_diffs=file_diffs,
                changed_files=changed_files,
                workspace_observation=workspace_observation,
                final_answer=f"Mock agent simulated failure for task {task.id}.",
                verification_result=None,
                error=error,
                failure_reason=failure_reason,
                failure_category=failure_category,
                model_rounds=int(mock.get("model_rounds", 0)),
                confirm_requests=int(mock.get("confirm_requests", 0)),
                repair_attempts_used=int(mock.get("repair_attempts_used", 0)),
                validation_attempts=int(mock.get("validation_attempts", 0)),
                input_tokens=mock.get("input_tokens"),
                output_tokens=mock.get("output_tokens"),
                cost_usd=mock.get("cost_usd"),
                forge_run_evidence=forge_run_evidence,
                started_at=started_at,
                ended_at=ended_at,
                duration_ms=int(mock.get("duration_ms", duration_ms(started_at, ended_at))),
            )

        verification_commands = task.validation_commands or (
            [task.verification_command] if task.verification_command is not None else []
        )
        if not verification_commands:
            error = "no_verification"
            failure_reason = "Task does not define a verification command."
            failure_category = FailureCategory.NO_VERIFICATION
        else:
            exit_code = 0 if task.expected_success else 1
            stdout = (
                "All verification checks passed." if task.expected_success else "1 test failed."
            )
            stderr = "" if task.expected_success else "AssertionError: simulated failure"
            for command in verification_commands:
                shell_outputs.append(
                    ShellOutput(
                        command=command,
                        stdout=stdout,
                        stderr=stderr,
                        exit_code=exit_code,
                        duration_ms=120,
                    )
                )
            verification_result = VerificationResult(
                command=verification_commands[-1],
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
            required_scores=task.required_scores,
            context_files=task.context_files,
            raw_events=list(mock.get("raw_events", [])),
            tool_calls=tool_calls,
            shell_outputs=shell_outputs,
            file_diffs=file_diffs,
            changed_files=changed_files,
            workspace_observation=workspace_observation,
            scope_violations=scope_violations,
            expected_files_changed=task.expected_files_changed,
            forbidden_files_changed=task.forbidden_files_changed,
            final_answer=final_answer,
            verification_result=verification_result,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            model_rounds=int(mock.get("model_rounds", 0)),
            confirm_requests=int(mock.get("confirm_requests", 0)),
            repair_attempts_used=int(mock.get("repair_attempts_used", 0)),
            validation_attempts=int(mock.get("validation_attempts", 0)),
            input_tokens=mock.get("input_tokens"),
            output_tokens=mock.get("output_tokens"),
            cost_usd=mock.get("cost_usd"),
            forge_run_evidence=forge_run_evidence,
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
        provider: EvalProvider | str = EvalProvider.FORGE,
        model: str = "local-forge",
        command: str | Sequence[str] | None = None,
        command_timeout_seconds: float = DEFAULT_FORGE_TIMEOUT_SECONDS,
        setup_timeout_seconds: float = DEFAULT_SETUP_TIMEOUT_SECONDS,
        validation_timeout_seconds: float = DEFAULT_VALIDATION_TIMEOUT_SECONDS,
        agent_provider: str | None = None,
        agent_model: str | None = None,
        agent_transport: str | None = None,
        agent_endpoint_family: str | None = None,
        agent_base_url: str | None = None,
        harness_fingerprint: str | None = None,
        max_model_rounds: int | None = None,
        max_repair_attempts: int | None = None,
        harness_policy: HarnessPolicyV1 | None = None,
        use_provider_credential_store: bool = False,
        execution_identity: ExecutionIdentityV1 | dict[str, Any] | None = None,
        child_env_overrides: Mapping[str, str] | None = None,
        api_key_env: str = "DEEPSEEK_API_KEY",
    ) -> None:
        self.provider = EvalProvider(provider)
        self.model = model
        self.command = normalize_command(command)
        self.command_timeout_seconds = command_timeout_seconds
        self.setup_timeout_seconds = setup_timeout_seconds
        self.validation_timeout_seconds = validation_timeout_seconds
        self.agent_provider = agent_provider or self.provider.value
        self.agent_model = agent_model or model
        self.agent_transport = agent_transport
        self.agent_endpoint_family = agent_endpoint_family
        self.agent_base_url = agent_base_url
        self.harness_fingerprint = harness_fingerprint
        self.max_model_rounds = max_model_rounds
        self.max_repair_attempts = max_repair_attempts
        self.harness_policy = harness_policy or HarnessPolicyV1()
        self.use_provider_credential_store = use_provider_credential_store
        self.child_env_overrides = _validate_child_env_overrides(child_env_overrides)
        if (
            not api_key_env
            or api_key_env[0].isdigit()
            or any(not (char.isalnum() or char == "_") for char in api_key_env)
        ):
            raise ValueError("api_key_env must be a valid environment variable name")
        self.api_key_env = api_key_env
        self.execution_identity = (
            ExecutionIdentityV1.model_validate(execution_identity)
            if execution_identity is not None
            else None
        )
        if self.execution_identity is not None:
            identity_route = (
                self.execution_identity.transport,
                self.execution_identity.endpoint_family,
                self.execution_identity.base_url,
            )
            runner_route = (
                self.agent_transport,
                self.agent_endpoint_family,
                self.agent_base_url,
            )
            if (
                self.execution_identity.provider != self.agent_provider
                or self.execution_identity.model != self.agent_model
                or identity_route != runner_route
            ):
                raise ValueError("execution identity does not match runner route")

    def credential_preflight(self) -> ForgeCredentialPreflight:
        if self.command is None:
            return self._credential_preflight_error("forge_command_not_configured")

        with tempfile.TemporaryDirectory(prefix="forge-eval-preflight-") as temp_dir:
            temp_root = Path(temp_dir)
            workspace = temp_root / "workspace"
            workspace.mkdir()
            runtime_state = temp_root / "runtime-state"
            runtime_home = runtime_state / "home"
            runtime_home.mkdir(parents=True)
            continuity_database_path = runtime_state / "continuity.db"
            try:
                self._write_isolated_provider_config(runtime_home)
            except (OSError, ValueError):
                return self._credential_preflight_error("isolated_provider_config_failed")
            child_env = os.environ.copy()
            original_home = child_env.get("HOME") or child_env.get("USERPROFILE")
            if original_home:
                child_env.setdefault("CARGO_HOME", str(Path(original_home) / ".cargo"))
                child_env.setdefault("RUSTUP_HOME", str(Path(original_home) / ".rustup"))
            child_env.update(
                {
                    "HOME": str(runtime_home),
                    "USERPROFILE": str(runtime_home),
                    "FORGE_EVAL_RUNTIME_STATE": str(runtime_state),
                    "XDG_CACHE_HOME": str(runtime_state / "cache"),
                    "BUN_INSTALL_CACHE_DIR": str(runtime_state / "bun-cache"),
                    "UV_CACHE_DIR": str(runtime_state / "uv-cache"),
                    "UV_NO_CONFIG": "1",
                    "UV_PROJECT_ENVIRONMENT": str(runtime_state / "uv-project-env"),
                }
            )
            child_env.update(self.child_env_overrides)
            payload = {
                "mode": "credential_preflight",
                "provider": self.agent_provider,
                "model": self.agent_model,
                "harness_policy": self.harness_policy.model_dump(mode="json"),
                "harness_policy_fingerprint": self.harness_policy.fingerprint(),
                "use_provider_credential_store": self.use_provider_credential_store,
                "workspace_path": str(workspace),
                "runtime_state_path": str(runtime_state),
                "continuity_database_path": str(continuity_database_path),
            }
            provider_route = self._provider_route_payload()
            if provider_route is not None:
                payload["provider_route"] = provider_route
            if self.execution_identity is not None:
                payload["execution_identity"] = self.execution_identity.model_dump(mode="json")
            try:
                completed = run_bounded_process(
                    self.command,
                    input_text=json.dumps(payload),
                    cwd=workspace,
                    timeout_seconds=min(self.command_timeout_seconds, 30.0),
                    cancel_requested=never_cancelled,
                    env=child_env,
                )
            except OSError:
                return self._credential_preflight_error("credential_preflight_command_failed")
            if completed.outcome == ProcessOutcome.TIMED_OUT:
                return self._credential_preflight_error("credential_preflight_timeout")
            if completed.outcome != ProcessOutcome.COMPLETED:
                return self._credential_preflight_error("credential_preflight_cancelled")
            if completed.returncode != 0:
                return self._credential_preflight_error("credential_preflight_command_failed")

            try:
                raw_payload = parse_forge_stdout(completed.stdout)
                result = ForgeCredentialPreflight.model_validate(
                    raw_payload["credential_preflight"]
                )
            except (KeyError, TypeError, ValidationError, ValueError, json.JSONDecodeError):
                return self._credential_preflight_error("credential_preflight_invalid_response")
            if (
                result.provider != self.agent_provider
                or result.model != self.agent_model
                or (self.agent_transport is not None and result.transport != self.agent_transport)
                or (
                    self.agent_endpoint_family is not None
                    and result.endpoint_family != self.agent_endpoint_family
                )
                or (self.agent_base_url is not None and result.base_url != self.agent_base_url)
            ):
                return self._credential_preflight_error("credential_preflight_identity_mismatch")
            return result

    def _credential_preflight_error(self, error: str) -> ForgeCredentialPreflight:
        return ForgeCredentialPreflight(
            ok=False,
            provider=self.agent_provider,
            model=self.agent_model,
            provider_call_started=False,
            error=error,
        )

    def _provider_route_payload(self) -> dict[str, str] | None:
        route_values = (
            self.agent_transport,
            self.agent_endpoint_family,
            self.agent_base_url,
        )
        if all(value is None for value in route_values):
            return None
        if any(value is None for value in route_values):
            raise ValueError("provider route identity is incomplete")
        transport = self.agent_transport
        base_url = self.agent_base_url
        assert transport is not None and base_url is not None
        return {"transport": transport, "base_url": base_url}

    def _write_isolated_provider_config(self, runtime_home: Path) -> None:
        route = self._provider_route_payload()
        if route is None:
            return
        config = {
            "providers": [
                {
                    "id": self.agent_provider,
                    "label": "Forge eval isolated provider",
                    "base_url": route["base_url"],
                    "api_key_env": self.api_key_env,
                    "default_model": self.agent_model,
                    "transport": route["transport"],
                    "supports_tools": True,
                    "supports_streaming": True,
                    **(
                        {"max_output_tokens_default": self.execution_identity.max_output_tokens}
                        if self.execution_identity is not None
                        else {}
                    ),
                }
            ]
        }
        config_path = runtime_home / ".forge" / "config.json"
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_bytes(
            (json.dumps(config, sort_keys=True, separators=(",", ":")) + "\n").encode()
        )

    def run_task(
        self,
        task: EvaluationTask,
        *,
        cancel_requested: CancelRequested = never_cancelled,
    ) -> AgentTrace:
        started_at = utc_now()
        if self.command is None:
            return self._error_trace(
                task=task,
                started_at=started_at,
                error="forge_command_not_configured",
                failure_reason="Set FORGE_EVAL_FORGE_AGENT_COMMAND to run provider=forge.",
                failure_category=FailureCategory.RUNNER_ERROR,
            )

        if cancel_requested():
            return self._error_trace(
                task=task,
                started_at=started_at,
                error="cancelled",
                failure_reason="Eval run was cancelled before task execution.",
                failure_category=FailureCategory.RUNNER_ERROR,
            )

        timeout = task.max_duration_seconds or self.command_timeout_seconds
        with tempfile.TemporaryDirectory(prefix=f"forge-eval-{task.id}-") as temp_dir:
            temp_root = Path(temp_dir)
            workspace = prepare_workspace(task, temp_root)
            if task.hidden_verifier_id is not None:
                try:
                    verifier_command(task.hidden_verifier_id, workspace)
                except ValueError as exc:
                    return self._error_trace(
                        task=task,
                        started_at=started_at,
                        error="hidden_verifier_invalid",
                        failure_reason=str(exc),
                        failure_category=FailureCategory.RUNNER_ERROR,
                    )
            runtime_state = temp_root / "runtime-state"
            runtime_home = runtime_state / "home"
            runtime_home.mkdir(parents=True, exist_ok=True)
            continuity_database_path = runtime_state / "continuity.db"
            try:
                self._write_isolated_provider_config(runtime_home)
            except (OSError, ValueError):
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="isolated_provider_config_failed",
                    failure_reason="Forge eval provider config could not be written.",
                    failure_category=FailureCategory.RUNNER_ERROR,
                )
            child_env = os.environ.copy()
            original_home = child_env.get("HOME") or child_env.get("USERPROFILE")
            if original_home:
                child_env.setdefault("CARGO_HOME", str(Path(original_home) / ".cargo"))
                child_env.setdefault("RUSTUP_HOME", str(Path(original_home) / ".rustup"))
            child_env.update(
                {
                    "HOME": str(runtime_home),
                    "USERPROFILE": str(runtime_home),
                    "FORGE_EVAL_RUNTIME_STATE": str(runtime_state),
                    "PYTHONDONTWRITEBYTECODE": "1",
                    "PYTHONPYCACHEPREFIX": str(runtime_state / "python-cache"),
                    "PYTEST_ADDOPTS": "-p no:cacheprovider",
                    "XDG_CACHE_HOME": str(runtime_state / "cache"),
                    "BUN_INSTALL_CACHE_DIR": str(runtime_state / "bun-cache"),
                    "UV_CACHE_DIR": str(runtime_state / "uv-cache"),
                    "UV_NO_CONFIG": "1",
                    "UV_PROJECT_ENVIRONMENT": str(runtime_state / "uv-project-env"),
                }
            )
            child_env.update(self.child_env_overrides)
            child_env = _install_uv_scope_wrapper(runtime_state, child_env)
            setup_outputs = run_setup_commands(
                task,
                workspace,
                timeout_seconds=self.setup_timeout_seconds,
                cancel_requested=cancel_requested,
                env=child_env,
            )
            setup_failure = next(
                (output for output in setup_outputs if output.exit_code != 0),
                None,
            )
            if setup_failure is not None:
                error, reason, category = command_failure(
                    setup_failure,
                    phase="Setup",
                    default_error="setup_failed",
                    default_category=FailureCategory.RUNNER_ERROR,
                )
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error=error,
                    failure_reason=reason,
                    failure_category=category,
                    shell_outputs=setup_outputs,
                )

            sandbox_scrub = scrub_future_repo_state(
                workspace,
                timeout_seconds=self.setup_timeout_seconds,
                cancel_requested=cancel_requested,
            )
            if not sandbox_scrub.ok:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="sandbox_scrub_failed",
                    failure_reason="Sandbox scrub failed: " + ", ".join(sandbox_scrub.findings),
                    failure_category=FailureCategory.RUNNER_ERROR,
                    shell_outputs=setup_outputs,
                    sandbox_scrub=sandbox_scrub,
                )

            try:
                before = snapshot_workspace(workspace)
            except OSError as exc:
                observation = WorkspaceObservation(
                    available=False,
                    source="filesystem_snapshot",
                    error=f"{type(exc).__name__}: {exc}",
                )
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="workspace_observation_unavailable",
                    failure_reason=f"Workspace snapshot failed: {type(exc).__name__}: {exc}",
                    failure_category=FailureCategory.RUNNER_ERROR,
                    shell_outputs=setup_outputs,
                    workspace_observation=observation,
                    sandbox_scrub=sandbox_scrub,
                )

            task_dict = task.model_dump(mode="json")
            for controller_owned_field in (
                "hidden_verifier_id",
                "full_verification_command",
                "scenario_path",
            ):
                task_dict.pop(controller_owned_field, None)
            # Map runner field names to Forge headless field names
            if task_dict.get("max_duration_seconds") is not None:
                task_dict["timeout_secs"] = task_dict.pop("max_duration_seconds")
            if self.max_model_rounds is not None:
                task_max_model_rounds = task_dict.get("max_model_rounds")
                task_dict["max_model_rounds"] = (
                    self.max_model_rounds
                    if task_max_model_rounds is None
                    else min(task_max_model_rounds, self.max_model_rounds)
                )
            if self.max_repair_attempts is not None:
                task_dict["max_repair_attempts"] = self.max_repair_attempts
            elif self.harness_policy.prompt_mode in {"minimal_v1", "planning_v1"}:
                task_dict["max_repair_attempts"] = self.harness_policy.repair_attempt_limit
            task_dict["stagnation_policy"] = self.harness_policy.stagnation_policy
            context_candidates = (
                _target_context_candidates(task, workspace)
                if self.harness_policy.context_policy == "target_files_v1"
                else task.context_files
            )
            prepared_context_files = self.harness_policy.selected_context_files(context_candidates)
            context_snapshot = _read_target_context_snapshot(
                workspace,
                prepared_context_files,
                enabled=self.harness_policy.context_policy == "target_files_v1",
            )
            task_dict["context_files"] = prepared_context_files
            context_manifest = build_context_manifest(
                prepared_context_files,
                workspace,
                mode=self.harness_policy.context_policy,
            )
            task_contract = build_task_execution_contract(
                expected_files_changed=task.expected_files_changed,
                forbidden_files_changed=task.forbidden_files_changed,
                context_files=prepared_context_files,
                validation_commands=task.validation_commands,
                verification_command=task.verification_command,
                tags=task.tags,
            )
            task_dict["task_execution_contract"] = task_contract.model_dump(mode="json")
            rendered_prompt = _render_eval_prompt(
                self.harness_policy,
                task,
                prepared_context_files,
                task_contract,
                context_snapshot=context_snapshot,
            )
            # Rust resolves the task prompt before the top-level request prompt.
            # Keep both controller-owned fields identical so eval-only policy and
            # bounded target context cannot be silently bypassed by that precedence.
            task_dict["prompt"] = rendered_prompt
            payload = {
                "task": task_dict,
                "prompt": rendered_prompt,
                "harness_policy": self.harness_policy.model_dump(mode="json"),
                "harness_policy_fingerprint": self.harness_policy.fingerprint(),
                "context_mode": self.harness_policy.context_policy,
                "context_manifest": context_manifest.model_dump(mode="json"),
                "context_manifest_fingerprint": context_manifest_fingerprint(context_manifest),
                "task_execution_contract": task_contract.model_dump(mode="json"),
                "provider": self.agent_provider,
                "model": self.agent_model,
                "use_provider_credential_store": self.use_provider_credential_store,
                "workspace_path": str(workspace),
                "runtime_state_path": str(runtime_state),
                "continuity_database_path": str(continuity_database_path),
                "write_policy": {
                    "allowed_files": list(task.expected_files_changed),
                    "forbidden_files": list(task.forbidden_files_changed),
                    "allowed_shell_commands": allowed_eval_shell_commands(task),
                },
            }
            provider_route = self._provider_route_payload()
            if provider_route is not None:
                payload["provider_route"] = provider_route
            if task.full_verification_command:
                # Keep the full controller check out of the task prompt and
                # visible tool allowlist. Rust may use its bounded failure
                # output for one repair turn, then the Python controller still
                # reruns the authoritative check after the child exits.
                payload["controller_verification_commands"] = [
                    task.full_verification_command
                ]
            if self.execution_identity is not None:
                payload["execution_identity"] = self.execution_identity.model_dump(mode="json")
            completed = run_bounded_process(
                self.command,
                input_text=json.dumps(payload),
                cwd=workspace,
                timeout_seconds=timeout,
                cancel_requested=cancel_requested,
                env=child_env,
            )
            command_output = ShellOutput(
                command=command_label(self.command),
                stdout=completed.stdout,
                stderr=completed.stderr,
                exit_code=completed.returncode,
                duration_ms=completed.duration_ms,
            )
            if completed.outcome != ProcessOutcome.COMPLETED:
                error = "timeout" if completed.outcome == ProcessOutcome.TIMED_OUT else "cancelled"
                reason = (
                    f"Forge command exceeded {timeout}s."
                    if completed.outcome == ProcessOutcome.TIMED_OUT
                    else "Eval run was cancelled during Forge execution."
                )
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error=error,
                    failure_reason=reason,
                    failure_category=(
                        FailureCategory.TIMEOUT
                        if completed.outcome == ProcessOutcome.TIMED_OUT
                        else FailureCategory.RUNNER_ERROR
                    ),
                    shell_outputs=[*setup_outputs, command_output],
                    sandbox_scrub=sandbox_scrub,
                )

            if completed.returncode != 0:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="forge_command_failed",
                    failure_reason=f"Forge command exited with {completed.returncode}.",
                    failure_category=FailureCategory.RUNNER_ERROR,
                    shell_outputs=[*setup_outputs, command_output],
                    sandbox_scrub=sandbox_scrub,
                )

            try:
                raw_payload = parse_forge_stdout(completed.stdout)
                reported_changed_files = reported_changed_files_from_payload(raw_payload)
                workspace_observation = observe_workspace_changes(
                    before,
                    workspace,
                    reported_changed_files=reported_changed_files,
                )
                trace = self._trace_from_payload(
                    task,
                    raw_payload,
                    started_at,
                    setup_outputs,
                    workspace,
                    child_env,
                    workspace_observation,
                    sandbox_scrub,
                    continuity_database_path,
                    cancel_requested,
                )
                trace = trace.model_copy(
                    update={
                        "raw_events": [
                            *trace.raw_events,
                            {
                                "event_type": "eval_runtime_isolation",
                                "runtime_state": "runtime-state",
                                "runtime_home": "runtime-state/home",
                                "continuity_database": "runtime-state/continuity.db",
                            },
                            {
                                "event_type": "eval_cache_isolation",
                                "cache_roots": {
                                    "xdg_cache_home": "runtime-state/cache",
                                    "bun_install_cache_dir": "runtime-state/bun-cache",
                                    "uv_cache_dir": "runtime-state/uv-cache",
                                },
                                "workspace_allowlist": [".bun-cache", ".uv-cache"],
                                "uv_run_scope_wrapper": True,
                            },
                        ]
                    }
                )
            except (json.JSONDecodeError, ValidationError, TypeError, ValueError) as exc:
                return self._error_trace(
                    task=task,
                    started_at=started_at,
                    error="invalid_forge_trace",
                    failure_reason=invalid_trace_failure_reason(
                        exc,
                        stdout=completed.stdout,
                        stderr=completed.stderr,
                    ),
                    failure_category=FailureCategory.FORGE_CONTRACT_ERROR,
                    shell_outputs=[*setup_outputs, command_output],
                    sandbox_scrub=sandbox_scrub,
                )

            patch_replay = self._replay_trace_patch(
                task,
                trace,
                temp_root,
                cancel_requested=cancel_requested,
            )
            raw_events = [
                *trace.raw_events,
                {
                    "event_type": "eval_patch_replay",
                    **patch_replay.model_dump(mode="json"),
                },
            ]
            return trace.model_copy(update={"patch_replay": patch_replay, "raw_events": raw_events})

    def _trace_from_payload(
        self,
        task: EvaluationTask,
        payload: dict[str, Any],
        started_at: datetime,
        setup_outputs: list[ShellOutput],
        workspace: Path,
        command_env: Mapping[str, str],
        workspace_observation: WorkspaceObservation,
        sandbox_scrub: LeakageCheck,
        continuity_database_path: Path | None,
        cancel_requested: CancelRequested,
    ) -> AgentTrace:
        ended_at = utc_now()
        final_answer = payload.get("final_answer")
        if not isinstance(final_answer, str):
            raise ValueError("Forge trace is missing required string field: final_answer")

        tool_calls = TypeAdapter(list[ShellOutput]).validate_python(payload.get("tool_calls", []))
        validation_outputs = run_validation_commands(
            task,
            workspace,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
            env=command_env,
        )
        regression_outputs = run_shell_commands(
            task.pass_to_pass_commands,
            workspace,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
            env=command_env,
        )
        fix_outputs = run_shell_commands(
            task.fail_to_pass_commands,
            workspace,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
            env=command_env,
        )
        post_validation_outputs = run_post_validation_commands(
            task,
            workspace,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
            env=command_env,
        )
        hidden_verifier_outputs = (
            [
                run_hidden_verifier(
                    task.hidden_verifier_id,
                    workspace,
                    timeout_seconds=self.validation_timeout_seconds,
                    cancel_requested=cancel_requested,
                )
            ]
            if task.hidden_verifier_id is not None
            else []
        )
        full_verification_outputs = run_shell_commands(
            (
                [task.full_verification_command]
                if task.full_verification_command is not None
                else []
            ),
            workspace,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
            env=command_env,
        )
        all_validation_outputs = [
            *validation_outputs,
            *regression_outputs,
            *fix_outputs,
            *post_validation_outputs,
            *hidden_verifier_outputs,
            *full_verification_outputs,
        ]
        shell_outputs = TypeAdapter(list[ShellOutput]).validate_python(
            [*setup_outputs, *payload.get("shell_outputs", []), *all_validation_outputs]
        )
        file_diffs = [
            diff
            for diff in TypeAdapter(list[FileDiff]).validate_python(payload.get("file_diffs", []))
            if not _is_ephemeral_cache_path(diff.path)
        ]
        raw_events = list(payload.get("raw_events", []))
        raw_events.append(
            _verification_ladder_event(
                validation_outputs=validation_outputs,
                regression_outputs=regression_outputs,
                fix_outputs=fix_outputs,
                post_validation_outputs=post_validation_outputs,
                hidden_verifier_outputs=hidden_verifier_outputs,
                full_verification_outputs=full_verification_outputs,
            )
        )
        validation_stop = payload.get("eval_validation_stop")
        if isinstance(validation_stop, dict):
            raw_events.append(
                {
                    "event_type": "eval_validation_stop",
                    **validation_stop,
                }
            )
        write_policy_blocks = payload.get("eval_write_policy_blocks", [])
        if isinstance(write_policy_blocks, list):
            raw_events.extend(
                block
                for block in write_policy_blocks
                if isinstance(block, dict) and block.get("event_type") == "eval_write_policy_block"
            )
        if self.execution_identity is not None and payload.get("error") is None:
            execution_event = next(
                (
                    event
                    for event in reversed(raw_events)
                    if isinstance(event, dict)
                    and event.get("event_type") == "eval_execution_identity"
                ),
                None,
            )
            validate_execution_identity_event(execution_event, expected=self.execution_identity)
        model_rounds = _reported_model_rounds(payload)
        derived_model_rounds = _model_rounds_from_agent_turn_events(raw_events)
        if derived_model_rounds is not None and model_rounds != derived_model_rounds:
            raise ValueError("model_rounds does not match authoritative agent_turn_updated events")
        verification_result = (
            VerificationResult.model_validate(payload["verification_result"])
            if payload.get("verification_result") is not None
            else None
        )
        forge_run_evidence = (
            ForgeRunEvidence.model_validate(payload["forge_run_evidence"])
            if payload.get("forge_run_evidence") is not None
            else None
        )
        cost_usd = payload.get("cost_usd")
        if cost_usd is None:
            usage_events = (
                forge_run_evidence.provider_usage.get("events")
                if forge_run_evidence is not None
                else None
            )
            if not isinstance(usage_events, list):
                usage_events = [
                    event
                    for event in raw_events
                    if isinstance(event, dict) and event.get("event_type") == "provider_usage"
                ]
            usage_events = [event for event in usage_events if isinstance(event, dict)]
            cost_usd = estimate_provider_cost_usd(usage_events)
            if is_kimi_k3_usage(usage_events):
                raw_events.append(kimi_k3_pricing_snapshot())
        if all_validation_outputs:
            last_validation = (
                first_failed_output(all_validation_outputs) or all_validation_outputs[-1]
            )
            verification_result = VerificationResult(
                command=last_validation.command,
                passed=last_validation.exit_code == 0,
                stdout=last_validation.stdout,
                stderr=last_validation.stderr,
                exit_code=last_validation.exit_code,
                duration_ms=last_validation.duration_ms,
            )
        changed_files = workspace_observation.changed_files
        context_candidates = (
            _target_context_candidates(task, workspace)
            if self.harness_policy.context_policy == "target_files_v1"
            else task.context_files
        )
        prepared_context_files = self.harness_policy.selected_context_files(context_candidates)
        task_contract = build_task_execution_contract(
            expected_files_changed=task.expected_files_changed,
            forbidden_files_changed=task.forbidden_files_changed,
            context_files=prepared_context_files,
            validation_commands=task.validation_commands,
            verification_command=task.verification_command,
            tags=task.tags,
        )
        identity_event = {
            "event_type": "eval_agent_identity",
            "provider": payload.get("forge_agent_provider"),
            "model": payload.get("forge_agent_model"),
            "transport": self.agent_transport,
            "harness_fingerprint": self.harness_fingerprint,
            "identity_matches_profile": (
                payload.get("forge_agent_provider") == self.agent_provider
                and payload.get("forge_agent_model") == self.agent_model
                and (
                    self.agent_transport is None
                    or payload.get("forge_agent_transport") == self.agent_transport
                )
                and (
                    self.agent_endpoint_family is None
                    or payload.get("forge_agent_endpoint_family") == self.agent_endpoint_family
                )
                and (
                    self.agent_base_url is None
                    or payload.get("forge_agent_base_url") == self.agent_base_url
                )
            ),
        }
        if self.agent_endpoint_family is not None:
            identity_event["endpoint_family"] = self.agent_endpoint_family
        if self.agent_base_url is not None:
            identity_event["base_url"] = self.agent_base_url
        raw_events.extend(
            [
                identity_event,
                {
                    "event_type": "eval_workspace_observation",
                    **workspace_observation.model_dump(mode="json"),
                },
                {
                    "event_type": "eval_sandbox_scrub",
                    **sandbox_scrub.model_dump(mode="json"),
                },
            ]
        )
        if task.pass_to_pass_commands or task.fail_to_pass_commands:
            raw_events.append(
                {
                    "event_type": "split_validation_commands",
                    "pass_to_pass_commands": task.pass_to_pass_commands,
                    "fail_to_pass_commands": task.fail_to_pass_commands,
                }
            )
        headless_continuity_diagnostic = headless_continuity_diagnostic_from_payload(payload)
        if headless_continuity_diagnostic is not None:
            raw_events.append(headless_continuity_diagnostic)
        continuity_diagnostic = continuity_db_diagnostic_at(
            continuity_database_path or workspace / ".forge" / "continuity.db",
            display_path=(
                "runtime-state/continuity.db"
                if continuity_database_path is not None
                else ".forge/continuity.db"
            ),
        )
        if continuity_diagnostic is not None:
            raw_events.append(continuity_diagnostic)
        scope_violations = scope_violations_for(task, changed_files)
        failure_category = normalize_failure_category(
            payload.get("failure_category", FailureCategory.NONE)
        )
        error = normalize_error(payload.get("error"), failure_category)
        failure_reason = payload.get("failure_reason")

        if (
            all_validation_outputs
            and verification_result is not None
            and verification_result.passed
            and failure_category == FailureCategory.VERIFICATION_FAILED
        ):
            error = None
            failure_reason = None
            failure_category = FailureCategory.NONE

        regression_failure = first_failed_output(regression_outputs)
        bugfix_failure = None if regression_failure else first_failed_output(fix_outputs)
        interrupted_validation = next(
            (output for output in all_validation_outputs if output.exit_code in {124, 130}),
            None,
        )
        if interrupted_validation is not None and failure_category == FailureCategory.NONE:
            error, failure_reason, failure_category = command_failure(
                interrupted_validation,
                phase="Validation",
                default_error="verification_failed",
                default_category=FailureCategory.VERIFICATION_FAILED,
            )
        elif regression_failure is not None and failure_category == FailureCategory.NONE:
            error = "verification_failed"
            failure_reason = "Regression validation failed"
            failure_category = FailureCategory.VERIFICATION_FAILED
        elif bugfix_failure is not None and failure_category == FailureCategory.NONE:
            error = "verification_failed"
            failure_reason = "Bug-fix validation failed"
            failure_category = FailureCategory.VERIFICATION_FAILED
        elif (
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

        try:
            repair_attempts_used = int(payload.get("repair_attempts_used", 0))
        except (TypeError, ValueError) as exc:
            raise ValueError("repair_attempts_used must be an integer") from exc
        if repair_attempts_used < 0:
            raise ValueError("repair_attempts_used must be nonnegative")
        if self.max_repair_attempts is not None and repair_attempts_used > self.max_repair_attempts:
            raise ValueError("repair_attempt_limit_exceeded")
        raw_events.append(
            {
                "event_type": "eval_verification_convergence",
                "repair_attempts_used": repair_attempts_used,
                "repair_attempt_limit": self.max_repair_attempts,
                "controller_verification_runs": len(all_validation_outputs),
                "fresh_controller_verifier": bool(all_validation_outputs),
                "single_repair_within_limit": (
                    self.max_repair_attempts is None
                    or repair_attempts_used <= self.max_repair_attempts
                ),
            }
        )
        raw_events.append(
            {
                "event_type": "eval_harness_policy",
                "policy": self.harness_policy.model_dump(mode="json"),
                "policy_fingerprint": self.harness_policy.fingerprint(),
                "prepared_context_files": prepared_context_files,
            }
        )

        if forge_run_evidence is not None:
            controller_shell_outputs = [
                {
                    "event_id": f"controller-verification-{index}",
                    "source": "controller_verification",
                    **output.model_dump(mode="json"),
                }
                for index, output in enumerate(all_validation_outputs, start=1)
            ]
            evidence_updates: dict[str, Any] = {
                "shell_outputs": [
                    *forge_run_evidence.shell_outputs,
                    *controller_shell_outputs,
                ],
                "failure_category": failure_category.value,
                "failure_reason": failure_reason,
            }
            if verification_result is not None:
                evidence_updates["verification"] = {
                    "agent_reported": forge_run_evidence.verification,
                    **verification_result.model_dump(mode="json"),
                }
            forge_run_evidence = forge_run_evidence.model_copy(update=evidence_updates)

        harness_evidence = harness_contract_evidence(
            task_context_files=prepared_context_files,
            workspace=workspace,
            tool_calls=(
                (call.command, call.stdout, call.stderr, call.exit_code) for call in tool_calls
            ),
            provided_envelopes=payload.get("tool_result_envelopes"),
            provided_signals=payload.get("progress_signals"),
            context_mode=self.harness_policy.context_policy,
            failure_category=failure_category.value,
            error=error,
            verification_passed=(
                verification_result.passed if verification_result is not None else None
            ),
            scope_violation=bool(scope_violations),
            task_contract=task_contract,
            provided_plan=payload.get("task_plan"),
            changed_files=changed_files,
        )
        raw_events.append(harness_evidence)

        return AgentTrace(
            task_id=task.id,
            user_prompt=task.prompt,
            model=self.model,
            provider=self.provider,
            required_scores=task.required_scores,
            context_files=prepared_context_files,
            raw_events=raw_events,
            tool_calls=tool_calls,
            shell_outputs=shell_outputs,
            file_diffs=file_diffs,
            changed_files=changed_files,
            scope_violations=scope_violations,
            expected_files_changed=task.expected_files_changed,
            forbidden_files_changed=task.forbidden_files_changed,
            final_answer=final_answer,
            verification_result=verification_result,
            error=error,
            failure_reason=failure_reason,
            failure_category=failure_category,
            model_rounds=model_rounds,
            confirm_requests=payload.get("confirm_requests", 0),
            repair_attempts_used=repair_attempts_used,
            validation_attempts=payload.get("validation_attempts", 0),
            input_tokens=payload.get("input_tokens"),
            output_tokens=payload.get("output_tokens"),
            cost_usd=cost_usd,
            forge_run_evidence=forge_run_evidence,
            workspace_observation=workspace_observation,
            sandbox_scrub=sandbox_scrub,
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )

    def _replay_trace_patch(
        self,
        task: EvaluationTask,
        trace: AgentTrace,
        temp_root: Path,
        *,
        cancel_requested: CancelRequested,
    ) -> WorkspaceCheck:
        if not trace.file_diffs:
            return WorkspaceCheck(
                ok=not trace.changed_files,
                modified_files=trace.changed_files if trace.changed_files else [],
                message=(
                    "Observed workspace changes have no replayable file diffs."
                    if trace.changed_files
                    else "No workspace changes to replay."
                ),
            )

        replay_workspace = prepare_workspace(task, temp_root, workspace_name="patch-replay")
        replay_setup = run_setup_commands(
            task,
            replay_workspace,
            timeout_seconds=self.setup_timeout_seconds,
            cancel_requested=cancel_requested,
        )
        setup_failure = first_failed_output(replay_setup)
        if setup_failure is not None:
            return WorkspaceCheck(
                ok=False,
                message=f"Patch replay setup failed: {setup_failure.command}",
            )
        return replay_patch(
            replay_workspace,
            trace.file_diffs,
            timeout_seconds=self.validation_timeout_seconds,
            cancel_requested=cancel_requested,
        )

    def _error_trace(
        self,
        *,
        task: EvaluationTask,
        started_at: datetime,
        error: str,
        failure_reason: str,
        failure_category: FailureCategory,
        shell_outputs: list[ShellOutput] | None = None,
        workspace_observation: WorkspaceObservation | None = None,
        sandbox_scrub: LeakageCheck | None = None,
        patch_replay: WorkspaceCheck | None = None,
    ) -> AgentTrace:
        ended_at = utc_now()
        return AgentTrace(
            task_id=task.id,
            user_prompt=task.prompt,
            model=self.model,
            provider=self.provider,
            required_scores=task.required_scores,
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
            workspace_observation=workspace_observation,
            sandbox_scrub=sandbox_scrub,
            patch_replay=patch_replay,
            repair_attempts_used=0,
            validation_attempts=0,
            started_at=started_at,
            ended_at=ended_at,
            duration_ms=duration_ms(started_at, ended_at),
        )


def validate_execution_identity(
    provider: EvalProvider | None,
    model: str | None,
    case_source: str | None,
) -> tuple[EvalProvider, str, str]:
    if provider is None:
        raise ValueError("Persisted eval run is missing provider")
    if model is None or not model.strip():
        raise ValueError("Persisted eval run is missing model")
    if case_source is None or not case_source.strip():
        raise ValueError("Persisted eval run is missing case_source")
    return provider, model, case_source


def create_runner(
    provider: EvalProvider | str,
    model: str,
    forge_command: str | Sequence[str] | None = None,
    command_timeout_seconds: float = DEFAULT_FORGE_TIMEOUT_SECONDS,
    setup_timeout_seconds: float = DEFAULT_SETUP_TIMEOUT_SECONDS,
    validation_timeout_seconds: float = DEFAULT_VALIDATION_TIMEOUT_SECONDS,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
) -> EvalRunner:
    try:
        normalized_provider = EvalProvider(provider)
    except ValueError as exc:
        raise ValueError(f"Unsupported eval provider: {provider}") from exc
    if normalized_provider == EvalProvider.MOCK:
        return DeterministicMockRunner(provider=normalized_provider, model=model)
    if normalized_provider == EvalProvider.FORGE:
        return ForgeAgentRunner(
            provider=normalized_provider,
            model=model,
            command=forge_command,
            command_timeout_seconds=command_timeout_seconds,
            setup_timeout_seconds=setup_timeout_seconds,
            validation_timeout_seconds=validation_timeout_seconds,
            max_model_rounds=max_model_rounds,
            harness_policy=harness_policy,
        )
    raise ValueError(f"Unsupported eval provider: {provider}")


def normalize_command(command: str | Sequence[str] | None) -> list[str] | None:
    if command is None:
        return None
    if isinstance(command, str):
        return shlex.split(command)
    return [str(part) for part in command]


def command_label(command: Sequence[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def parse_forge_stdout(stdout: str) -> dict[str, Any]:
    text = stdout.strip()
    if not text:
        return {}
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError:
        start = text.rfind("\n{")
        if start != -1:
            parsed = json.loads(text[start + 1 :])
        else:
            start = text.find("{")
            if start == -1:
                raise
            parsed = json.loads(text[start:])
    if not isinstance(parsed, dict):
        raise TypeError("Forge command stdout must contain a JSON object.")
    return parsed


def _reported_model_rounds(payload: dict[str, Any]) -> int:
    value = payload.get("model_rounds", 0)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError("model_rounds must be a nonnegative integer")
    return value


def _model_rounds_from_agent_turn_events(
    raw_events: Sequence[dict[str, Any]],
) -> int | None:
    """Rebuild rounds when the canonical headless event stream is present.

    External runners and legacy fixtures may only provide a summary, so this
    remains an optional cross-check.  Forge's headless trace emits
    ``agent_turn_updated`` projections with a ``calling_model`` status; those
    transitions are the authoritative round boundaries and must agree with the
    summary field used by the release gate.
    """

    saw_turn_projection = False
    transitions = 0
    previous_calling_model = False
    for event in raw_events:
        if not isinstance(event, dict) or event.get("event_type") != "agent_turn_updated":
            continue
        saw_turn_projection = True
        state = event.get("state")
        status = state.get("status") if isinstance(state, dict) else None
        calling_model = status == "calling_model"
        if calling_model and not previous_calling_model:
            transitions += 1
        previous_calling_model = calling_model
    return transitions if saw_turn_projection else None


def reported_changed_files_from_payload(payload: dict[str, Any]) -> list[str]:
    changed_files = payload.get("changed_files")
    if isinstance(changed_files, list):
        return [str(path) for path in changed_files]
    file_diffs = payload.get("file_diffs")
    if not isinstance(file_diffs, list):
        return []
    return [
        str(diff["path"])
        for diff in file_diffs
        if isinstance(diff, dict) and diff.get("path") is not None
    ]


def invalid_trace_failure_reason(exc: Exception, *, stdout: str, stderr: str) -> str:
    reason = f"Forge command returned invalid trace JSON: {type(exc).__name__}: {exc}"
    stdout_preview = text_preview(stdout, 500)
    stderr_preview = text_preview(stderr, 500)
    if stdout_preview:
        reason += f" | stdout preview: {stdout_preview}"
    if stderr_preview:
        reason += f" | stderr preview: {stderr_preview}"
    return reason


def mock_metadata(task: EvaluationTask) -> dict[str, Any]:
    raw_mock = task.metadata.get("mock", {})
    return raw_mock if isinstance(raw_mock, dict) else {}


def mock_forge_run_evidence(mock: dict[str, Any]) -> ForgeRunEvidence | None:
    raw_evidence = mock.get("forge_run_evidence")
    if raw_evidence is None:
        return None
    return ForgeRunEvidence.model_validate(raw_evidence)


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


def _is_ephemeral_cache_path(path: str) -> bool:
    return any(part in EPHEMERAL_CACHE_DIRS for part in Path(path).parts)


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


def prepare_workspace(
    task: EvaluationTask,
    temp_dir: Path,
    *,
    workspace_name: str = "workspace",
) -> Path:
    workspace = temp_dir / workspace_name
    if task.fixture_path is None:
        workspace.mkdir(parents=True, exist_ok=True)
    else:
        fixture = Path(task.fixture_path).expanduser()
        if fixture.exists():
            shutil.copytree(fixture, workspace)
        else:
            workspace.mkdir(parents=True, exist_ok=True)

    if task.scenario_path is not None:
        apply_scenario_overlay(Path(task.scenario_path).expanduser(), workspace)
    return workspace


def apply_scenario_overlay(scenario: Path, workspace: Path) -> None:
    if scenario.is_symlink():
        raise ValueError("scenario symlinks are forbidden: .")
    if not scenario.is_dir():
        raise ValueError(f"scenario overlay does not exist: {scenario}")

    workspace_root = workspace.resolve()
    for source in sorted(
        scenario.rglob("*"),
        key=lambda path: path.relative_to(scenario).as_posix(),
    ):
        relative = source.relative_to(scenario)
        if source.is_symlink():
            raise ValueError(f"scenario symlinks are forbidden: {relative.as_posix()}")
        target = workspace / relative
        resolved_target = target.resolve(strict=False)
        if not resolved_target.is_relative_to(workspace_root):
            raise ValueError(f"scenario path escapes workspace: {relative.as_posix()}")
        if target.is_symlink():
            raise ValueError(f"scenario target symlinks are forbidden: {relative.as_posix()}")
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        else:
            raise ValueError(f"scenario contains unsupported entry: {relative.as_posix()}")


def continuity_db_diagnostic(workspace: Path) -> dict[str, Any] | None:
    return continuity_db_diagnostic_at(
        workspace / ".forge" / "continuity.db",
        display_path=".forge/continuity.db",
    )


def continuity_db_diagnostic_at(
    db_path: Path,
    *,
    display_path: str,
) -> dict[str, Any] | None:
    if not db_path.exists():
        return None

    diagnostic: dict[str, Any] = {
        "event_type": "eval_continuity_db_diagnostic",
        "exists": True,
        "db_path": display_path,
    }
    try:
        conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        try:
            diagnostic["event_counts"] = continuity_event_counts(conn)
            diagnostic["experience_count"] = count_table_rows(conn, "continuity_experiences")
            diagnostic["experience_status_counts"] = count_table_column_values(
                conn, "continuity_experiences", "status"
            )
            diagnostic["experience_kind_counts"] = count_table_column_values(
                conn, "continuity_experiences", "kind"
            )
            diagnostic["fts_count"] = count_table_rows(conn, "continuity_experiences_fts")
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
        "SELECT event_json FROM continuity_events WHERE event_type = 'reflection' LIMIT 5"
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


def count_table_column_values(conn: sqlite3.Connection, table: str, column: str) -> dict[str, int]:
    if not sqlite_table_exists(conn, table) or not sqlite_column_exists(conn, table, column):
        return {}
    rows = conn.execute(
        f"SELECT {column}, COUNT(*) FROM {table} "
        f"WHERE {column} IS NOT NULL AND {column} != '' GROUP BY {column}"
    ).fetchall()
    return {str(value): int(count) for value, count in rows}


def sqlite_column_exists(conn: sqlite3.Connection, table: str, column: str) -> bool:
    rows = conn.execute(f"PRAGMA table_info({table})").fetchall()
    return any(row[1] == column for row in rows)


def sqlite_table_exists(conn: sqlite3.Connection, table: str) -> bool:
    row = conn.execute(
        "SELECT COUNT(1) FROM sqlite_master WHERE type = 'table' AND name = ?",
        (table,),
    ).fetchone()
    return bool(row and row[0])


def run_setup_commands(
    task: EvaluationTask,
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    env: Mapping[str, str] | None = None,
) -> list[ShellOutput]:
    return run_shell_commands(
        task.setup_commands,
        workspace,
        timeout_seconds=timeout_seconds,
        cancel_requested=cancel_requested,
        env=env,
    )


def run_validation_commands(
    task: EvaluationTask,
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    env: Mapping[str, str] | None = None,
) -> list[ShellOutput]:
    return run_shell_commands(
        task.validation_commands,
        workspace,
        timeout_seconds=timeout_seconds,
        cancel_requested=cancel_requested,
        env=env,
    )


def run_post_validation_commands(
    task: EvaluationTask,
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    env: Mapping[str, str] | None = None,
) -> list[ShellOutput]:
    return run_shell_commands(
        task.post_validation_commands,
        workspace,
        timeout_seconds=timeout_seconds,
        cancel_requested=cancel_requested,
        env=env,
    )


def first_failed_output(outputs: Sequence[ShellOutput]) -> ShellOutput | None:
    return next((output for output in outputs if output.exit_code != 0), None)


def _verification_ladder_event(
    *,
    validation_outputs: Sequence[ShellOutput],
    regression_outputs: Sequence[ShellOutput],
    fix_outputs: Sequence[ShellOutput],
    post_validation_outputs: Sequence[ShellOutput],
    hidden_verifier_outputs: Sequence[ShellOutput],
    full_verification_outputs: Sequence[ShellOutput],
) -> dict[str, Any]:
    """Record which verification layer first rejected an otherwise valid run."""

    output_groups = (
        ("task_validation", validation_outputs),
        ("regression", regression_outputs),
        ("fix_validation", fix_outputs),
        ("post_validation", post_validation_outputs),
        ("hidden_verifier", hidden_verifier_outputs),
        ("full_verification", full_verification_outputs),
    )
    stages: list[dict[str, Any]] = []
    for name, outputs in output_groups:
        stages.append(
            {
                "stage": name,
                "present": bool(outputs),
                "passed": (
                    all(output.exit_code == 0 for output in outputs) if outputs else None
                ),
                "attempts": len(outputs),
                "commands": [
                    {"command": output.command, "exit_code": output.exit_code}
                    for output in outputs
                ],
            }
        )

    first_failed_stage = next(
        (
            stage["stage"]
            for stage in stages
            if stage["present"] and stage["passed"] is False
        ),
        None,
    )
    controller_outputs = [
        *post_validation_outputs,
        *hidden_verifier_outputs,
        *full_verification_outputs,
    ]
    return {
        "event_type": "eval_verification_ladder",
        "schema_version": 1,
        "stages": stages,
        "first_failed_stage": first_failed_stage,
        "target_validation_passed": (
            all(output.exit_code == 0 for output in validation_outputs)
            if validation_outputs
            else None
        ),
        "controller_verification_passed": (
            all(output.exit_code == 0 for output in controller_outputs)
            if controller_outputs
            else None
        ),
    }


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
        "budget_exhausted": FailureCategory.BUDGET_EXHAUSTED,
        "max_model_rounds_exceeded": FailureCategory.BUDGET_EXHAUSTED,
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


def run_shell_commands(
    commands: Sequence[str],
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    env: Mapping[str, str] | None = None,
) -> list[ShellOutput]:
    outputs: list[ShellOutput] = []
    for command in commands:
        completed = run_bounded_process(
            command,
            cwd=workspace,
            timeout_seconds=timeout_seconds,
            cancel_requested=cancel_requested,
            shell=True,
            env=env,
        )
        outputs.append(
            ShellOutput(
                command=command,
                stdout=completed.stdout,
                stderr=completed.stderr,
                exit_code=completed.returncode,
                duration_ms=completed.duration_ms,
            )
        )
        if completed.outcome != ProcessOutcome.COMPLETED or completed.returncode != 0:
            break
    return outputs


def allowed_eval_shell_commands(task: EvaluationTask) -> list[str]:
    """Return controller-approved commands the agent may run during an eval.

    Headless evals auto-resolve confirmations, but the write boundary must not
    turn that into unrestricted shell access.  Exact task-owned validation and
    regression commands are the narrowest useful allowlist: they let the agent
    verify its patch while keeping every other shell confirmation fail-closed.
    """

    commands = [
        *task.validation_commands,
        *task.pass_to_pass_commands,
        *task.fail_to_pass_commands,
    ]
    if not task.validation_commands and task.verification_command is not None:
        commands.append(task.verification_command)
    return list(dict.fromkeys(command.strip() for command in commands if command.strip()))


def command_failure(
    output: ShellOutput,
    *,
    phase: str,
    default_error: str,
    default_category: FailureCategory,
) -> tuple[str, str, FailureCategory]:
    if output.exit_code == 124:
        return (
            "timeout",
            f"{phase} command timed out: {output.command}",
            FailureCategory.TIMEOUT,
        )
    if output.exit_code == 130:
        return (
            "cancelled",
            f"{phase} command was cancelled: {output.command}",
            FailureCategory.RUNNER_ERROR,
        )
    return (
        default_error,
        f"{phase} command failed: {output.command}",
        default_category,
    )


def scope_violations_for(task: EvaluationTask, changed_files: Sequence[str]) -> list[str]:
    expected = set(task.expected_files_changed)
    forbidden = set(task.forbidden_files_changed)
    expected_no_change = task.metadata.get("expected_no_change") is True
    violations: list[str] = []
    for path in changed_files:
        if any(fnmatchcase(path, pattern) for pattern in forbidden):
            violations.append(f"forbidden_change:{path}")
        if (expected or expected_no_change) and path not in expected:
            violations.append(f"unexpected_change:{path}")
    return violations


def _render_eval_prompt(
    policy: HarnessPolicyV1,
    task: EvaluationTask,
    context_files: Sequence[str],
    task_contract: Any | None = None,
    *,
    context_snapshot: Mapping[str, str] | None = None,
) -> str:
    """Add controller-owned scope and convergence facts to the agent prompt.

    The task JSON already carries these fields, but the desktop headless request
    intentionally accepts only the agent-facing subset.  Rendering the bounded
    scope contract here makes the evaluator's authority visible before the first
    edit, rather than discovering an out-of-scope patch only after the run.
    """

    prompt = policy.render_prompt(
        task.prompt,
        context_files,
        context_snapshot=context_snapshot,
    )
    if task_contract is None:
        task_contract = build_task_execution_contract(
            expected_files_changed=task.expected_files_changed,
            forbidden_files_changed=task.forbidden_files_changed,
            context_files=context_files,
            validation_commands=task.validation_commands,
            verification_command=task.verification_command,
            tags=task.tags,
        )
    expected = ", ".join(task.expected_files_changed) or "(none; no product file should change)"
    forbidden = ", ".join(task.forbidden_files_changed) or "(none declared)"
    scope_contract = (
        "\n\nController scope contract:\n"
        f"- Allowed changed files: {expected}\n"
        "- HARD boundary: edit only the exact allowed paths; do not broaden the patch.\n"
        f"- Forbidden changed paths: {forbidden}\n"
        "- Do not modify any other file; keep the patch minimal.\n"
        "- Do not edit adjacent implementation or test files to make validation pass; if a "
        "dependency looks missing, stop and report it.\n"
        "- Treat tests, fixtures, package manifests, lockfiles, virtualenvs, caches, and build "
        "outputs as read-only; do not create or edit .venv, uv.lock, node_modules, or cache "
        "files.\n"
        "- Do not create helper files such as .cache-* or tools/*; use the task's existing "
        "validation commands instead of improvised repo-local scripts.\n"
        "- If a command generates a lockfile or temporary helper, remove it before final "
        "verification.\n"
        "- Run the targeted validation, then stop once the task is verified.\n"
        "- Use the exact required targeted check command; do not append files, redirections, "
        "pipes, or other shell commands.\n"
        "- Target at most 12 model rounds: inspect early, make the smallest edit, validate, "
        "and stop; do not spend rounds polishing an already verified result.\n"
        "- Do not repeat an unchanged read-only command; only rerun it after new evidence or "
        "a file change."
    )
    required_inspection = ", ".join(task_contract.required_inspection_files) or "(none declared)"
    required_validation = (
        ", ".join(command[:240] for command in task_contract.required_validation_commands)
        or "(none declared)"
    )
    planning_contract = (
        "\n\nController execution contract:\n"
        f"- Task shape: {task_contract.kind.value}.\n"
        f"- Inspect before editing: {required_inspection}.\n"
        f"- Required targeted checks: {required_validation}.\n"
        f"- Stop condition: {task_contract.stop_condition}\n"
        "- Before the first mutation, form a short inspect/edit/verify plan; if the "
        "headless adapter supports it, return it as task_plan with at most 8 steps "
        "using {goal, steps:[{id, action, files, evidence}]} and actions inspect/edit/verify.\n"
        "- A no_change task must not contain an edit step."
    )
    return prompt + scope_contract + planning_contract


def _read_target_context_snapshot(
    workspace: Path,
    paths: Sequence[str],
    *,
    enabled: bool,
) -> dict[str, str]:
    """Read only controller-selected in-workspace files for target context."""

    if not enabled:
        return {}
    workspace_root = workspace.resolve()
    snapshot: dict[str, str] = {}
    for raw_path in paths:
        relative = Path(raw_path)
        if relative.is_absolute() or ".." in relative.parts:
            continue
        candidate = (workspace_root / relative).resolve()
        if candidate != workspace_root and workspace_root not in candidate.parents:
            continue
        try:
            with candidate.open("r", encoding="utf-8", errors="replace") as handle:
                snapshot[raw_path] = handle.read(4_001)
        except (OSError, UnicodeError):
            continue
    return snapshot


_VALIDATION_CONTEXT_PATH = re.compile(
    r"(?<![A-Za-z0-9_./-])([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.(?:py|ts|tsx|js|jsx|json|md))"
)


def _target_context_candidates(task: EvaluationTask, workspace: Path) -> list[str]:
    """Select allowed targets plus visible validation files for target context."""

    candidates: list[str] = []
    forbidden = task.forbidden_files_changed

    def add(path: str, *, require_file: bool) -> None:
        normalized = path.strip()
        relative = Path(normalized)
        if (
            not normalized
            or relative.is_absolute()
            or ".." in relative.parts
            or any(fnmatchcase(normalized, pattern) for pattern in forbidden)
            or normalized in candidates
        ):
            return
        candidate = (workspace.resolve() / relative).resolve()
        root = workspace.resolve()
        if candidate != root and root not in candidate.parents:
            return
        if require_file and not candidate.is_file():
            return
        candidates.append(normalized)

    for path in [*task.context_files, *task.expected_files_changed]:
        add(path, require_file=False)
    commands = [
        *task.validation_commands,
        *task.pass_to_pass_commands,
        *task.fail_to_pass_commands,
        task.verification_command or "",
    ]
    for command in commands:
        for path in _VALIDATION_CONTEXT_PATH.findall(command):
            add(path, require_file=True)
    return candidates
