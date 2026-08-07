from __future__ import annotations

import os
import shlex
import signal
import subprocess
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

from app.models import ProcessOutcome


@dataclass(frozen=True)
class BoundedProcessResult:
    command: str
    stdout: str
    stderr: str
    returncode: int
    duration_ms: int
    outcome: ProcessOutcome


CancelRequested = Callable[[], bool]


def never_cancelled() -> bool:
    return False


def run_bounded_process(
    command: str | Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    input_text: str | None = None,
    shell: bool = False,
    env: Mapping[str, str] | None = None,
) -> BoundedProcessResult:
    started = time.monotonic()
    launch_command: str | Sequence[str]
    if isinstance(command, str) and not shell:
        launch_command = shlex.split(command)
        if not launch_command:
            raise ValueError("process command must not be empty")
    else:
        launch_command = command
    process = subprocess.Popen(
        launch_command,
        cwd=cwd,
        shell=shell,
        text=True,
        stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
        env=dict(env) if env is not None else None,
    )
    deadline = started + timeout_seconds
    pending_input = input_text

    while True:
        if cancel_requested():
            stdout, stderr = _terminate_process_group(process)
            return _result(
                command,
                stdout,
                stderr,
                130,
                started,
                ProcessOutcome.CANCELLED,
            )

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            stdout, stderr = _terminate_process_group(process)
            return _result(
                command,
                stdout,
                stderr,
                124,
                started,
                ProcessOutcome.TIMED_OUT,
            )

        try:
            stdout, stderr = process.communicate(
                input=pending_input,
                timeout=min(0.05, remaining),
            )
            return _result(
                command,
                stdout,
                stderr,
                process.returncode,
                started,
                ProcessOutcome.COMPLETED,
            )
        except subprocess.TimeoutExpired:
            pending_input = None


def _terminate_process_group(process: subprocess.Popen[str]) -> tuple[str, str]:
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except PermissionError:
            process.terminate()
    try:
        return process.communicate(timeout=0.5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except PermissionError:
            process.kill()
        return process.communicate()


def _result(
    command: str | Sequence[str],
    stdout: str,
    stderr: str,
    returncode: int,
    started: float,
    outcome: ProcessOutcome,
) -> BoundedProcessResult:
    label = command if isinstance(command, str) else " ".join(command)
    return BoundedProcessResult(
        command=label,
        stdout=stdout,
        stderr=stderr,
        returncode=returncode,
        duration_ms=max(0, int((time.monotonic() - started) * 1000)),
        outcome=outcome,
    )
