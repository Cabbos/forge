import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COMMANDS = [
    ["bun", "run", "test:ts"],
    [
        "uv",
        "run",
        "--no-project",
        "--with",
        "pytest==9.0.3",
        "pytest",
        "tests/baseline/test_task_api.py",
        "tests/baseline/test_worker.py",
        "tests/integration/test_task_lifecycle.py",
        "-q",
    ],
    ["bun", "run", "typecheck"],
    [sys.executable, "-m", "compileall", "-q", "services"],
]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgebench-runtime-") as runtime_state:
        environment = os.environ.copy()
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        environment["PYTHONPYCACHEPREFIX"] = str(Path(runtime_state) / "pycache")
        environment["PYTEST_ADDOPTS"] = "-p no:cacheprovider"
        for command in COMMANDS:
            completed = subprocess.run(command, cwd=ROOT, env=environment, check=False)
            if completed.returncode != 0:
                return completed.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
