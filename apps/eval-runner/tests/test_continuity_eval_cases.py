import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CASES_ROOT = ROOT / "eval_cases"


def load_continuity_case_tasks() -> list[dict]:
    tasks: list[dict] = []
    for case_path in sorted(CASES_ROOT.glob("continuity-pipeline-*/case.json")):
        payload = json.loads(case_path.read_text(encoding="utf-8"))
        task = payload.get("task", payload)
        task["_case_path"] = str(case_path.relative_to(ROOT))
        tasks.append(task)
    return tasks


def test_continuity_stress_suite_has_at_least_ten_cases() -> None:
    tasks = load_continuity_case_tasks()

    assert len(tasks) >= 10


def test_continuity_cases_run_business_and_sqlite_post_validation() -> None:
    tasks = load_continuity_case_tasks()

    for task in tasks:
        post_validation = task.get("post_validation_commands", [])
        joined = "\n".join(post_validation)
        forbidden_files = set(task.get("forbidden_files_changed", []))

        assert task["id"].startswith("continuity-pipeline-"), task["_case_path"]
        assert task.get("fixture_path") == "../_fixtures/continuity-ts-tooling", task["_case_path"]
        assert "npm install" in task.get("setup_commands", []), task["_case_path"]
        assert task.get("validation_commands", []) == [], task["_case_path"]
        assert "npm test" in post_validation, task["_case_path"]
        assert "npx tsc --noEmit" in post_validation, task["_case_path"]
        assert "scripts/assert-continuity.py" in joined, task["_case_path"]
        assert ".forge/continuity.db" not in forbidden_files, task["_case_path"]
        assert ".env" in forbidden_files, task["_case_path"]
        assert "package-lock.json" in forbidden_files, task["_case_path"]
