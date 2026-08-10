from pathlib import Path
from typing import Any

from _support import run_python_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import os
import sys
from pathlib import Path
import pytest

root = Path(os.environ["FORGEBENCH_WORKSPACE"])
sys.path.insert(0, str(root / "services" / "event-worker"))
from event_worker.retry import retry_delay_seconds

def test_retry_curve_uses_one_based_attempts_and_exact_cap():
    assert [retry_delay_seconds(i, base=0.25, cap=2) for i in range(1, 7)] == [
        0.25, 0.5, 1.0, 2.0, 2.0, 2.0
    ]
    with pytest.raises(ValueError):
        retry_delay_seconds(0)
"""
    return {"python": run_python_test(workspace, source)}


verify("dev-worker-retry-backoff", check)
