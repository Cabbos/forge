import sys
import threading
import time
from pathlib import Path

from app.models import ProcessOutcome
from app.process_control import run_bounded_process


def test_bounded_process_returns_completed_output(tmp_path: Path) -> None:
    result = run_bounded_process(
        [sys.executable, "-c", "print('ok')"],
        cwd=tmp_path,
        timeout_seconds=2.0,
    )

    assert result.outcome == ProcessOutcome.COMPLETED
    assert result.returncode == 0
    assert result.stdout == "ok\n"


def test_bounded_process_timeout_preserves_partial_output(tmp_path: Path) -> None:
    result = run_bounded_process(
        [sys.executable, "-u", "-c", "import time; print('started'); time.sleep(10)"],
        cwd=tmp_path,
        timeout_seconds=0.2,
    )

    assert result.outcome == ProcessOutcome.TIMED_OUT
    assert result.returncode == 124
    assert "started" in result.stdout


def test_bounded_process_cancellation_kills_descendant(tmp_path: Path) -> None:
    marker = tmp_path / "child-finished"
    child_code = (
        "import time,pathlib; time.sleep(2); "
        f"pathlib.Path(r'{marker}').write_text('x')"
    )
    cancelled = threading.Event()
    timer = threading.Timer(0.2, cancelled.set)
    timer.start()
    try:
        result = run_bounded_process(
            [
                sys.executable,
                "-c",
                (
                    "import subprocess,sys,time; "
                    f"subprocess.Popen([sys.executable,'-c',{child_code!r}]); "
                    "time.sleep(10)"
                ),
            ],
            cwd=tmp_path,
            timeout_seconds=5.0,
            cancel_requested=cancelled.is_set,
        )
    finally:
        timer.cancel()

    assert result.outcome == ProcessOutcome.CANCELLED
    assert result.returncode == 130
    time.sleep(2.2)
    assert not marker.exists()
