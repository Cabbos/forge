from pathlib import Path
from typing import Any

from _support import assert_workspace_path, verify


def assert_baseline_health(workspace: Path) -> dict[str, Any]:
    required = (
        "package.json",
        "pyproject.toml",
        "apps/web-console/src/main.tsx",
        "services/task-api/task_api/service.py",
        "tools/release/verify.py",
    )
    missing = [path for path in required if not assert_workspace_path(workspace, path).is_file()]
    if missing:
        raise AssertionError(f"missing baseline files: {missing}")
    return {"required_file_count": len(required)}


verify("baseline-health", assert_baseline_health)
