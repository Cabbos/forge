import argparse
import json
import os
import tempfile
from pathlib import Path
from typing import Any

from app.artifact_hashing import IGNORED_TREE_PARTS, canonical_tree_digest
from app.cases import load_cases, validate_case_quality
from app.datasets import VERIFIER_ROOT, dataset_fingerprint
from app.models import DatasetSplit, EvaluationTask

FIXTURE_ID = "forgebench-monorepo"
FIXTURE_VERSION = "1.0.0"
FIXTURE_VERIFICATION_COMMAND = "python3 tools/release/verify.py"
FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
DEFAULT_CASES = FORGEBENCH_ROOT / "cases"
DEFAULT_DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"


def freeze_fixture(fixture: Path, output: Path) -> dict[str, Any]:
    _assert_fixture_is_clean(fixture)
    payload = {
        "schema_version": 1,
        "fixture_id": FIXTURE_ID,
        "fixture_version": FIXTURE_VERSION,
        "canonical_tree_sha256": canonical_tree_digest(fixture),
        "verification_command": FIXTURE_VERIFICATION_COMMAND,
    }
    _atomic_write_json(output, payload)
    return payload


def freeze_dataset(cases: Path, fixture_lock: Path, output: Path) -> dict[str, Any]:
    tasks = _load_valid_catalog(cases)
    fixture = _load_fixture_identity(fixture_lock)
    _assert_catalog_uses_fixture(tasks, fixture)
    payload = _dataset_lock_payload(tasks, fixture)
    _atomic_write_json(output, payload)
    return payload


def verify_catalog(cases: Path, lock: Path) -> dict[str, Any]:
    tasks = _load_valid_catalog(cases)
    actual = json.loads(lock.read_text(encoding="utf-8"))
    fixture = _fixture_identity_from_tasks(tasks)
    expected = _dataset_lock_payload(tasks, fixture)
    if actual != expected:
        raise ValueError("dataset lock does not match catalog contents")
    return _catalog_summary(expected)


def show_lock(lock: Path, split: DatasetSplit) -> dict[str, Any]:
    payload = json.loads(lock.read_text(encoding="utf-8"))
    if payload.get("schema_version") != 1:
        raise ValueError("unsupported ForgeBench dataset lock schema")
    split_payload = payload.get(split.value)
    if not isinstance(split_payload, dict):
        raise ValueError(f"dataset lock is missing {split.value} split")
    return {
        "split": split.value,
        "count": split_payload["count"],
        "ids": split_payload["ids"],
        "fingerprint": split_payload["fingerprint"],
    }


def _load_valid_catalog(cases: Path) -> list[EvaluationTask]:
    tasks = load_cases(cases)
    issues = validate_case_quality(tasks)
    if issues:
        rendered = ", ".join(
            f"{issue.task_id}:{issue.code}"
            for issue in sorted(issues, key=lambda item: item.task_id)
        )
        raise ValueError(f"ForgeBench catalog has quality issues: {rendered}")

    development = [task for task in tasks if task.dataset_split == DatasetSplit.DEVELOPMENT]
    holdout = [task for task in tasks if task.dataset_split == DatasetSplit.HOLDOUT]
    if len(tasks) != 30 or len(development) != 10 or len(holdout) != 20:
        raise ValueError(
            "ForgeBench catalog must contain exactly 10 development and 20 holdout cases"
        )
    return tasks


def _load_fixture_identity(fixture_lock: Path) -> dict[str, str]:
    payload = json.loads(fixture_lock.read_text(encoding="utf-8"))
    try:
        fixture = {
            "fixture_id": str(payload["fixture_id"]),
            "fixture_version": str(payload["fixture_version"]),
            "canonical_tree_sha256": str(payload["canonical_tree_sha256"]),
        }
    except KeyError as exc:
        raise ValueError(f"fixture lock is missing {exc.args[0]}") from exc
    if fixture["fixture_id"] != FIXTURE_ID or fixture["fixture_version"] != FIXTURE_VERSION:
        raise ValueError("fixture lock identity does not match ForgeBench")
    return fixture


def _fixture_identity_from_tasks(tasks: list[EvaluationTask]) -> dict[str, str]:
    fixture_ids = {task.fixture_id for task in tasks}
    fixture_versions = {task.fixture_version for task in tasks}
    fixture_paths = {task.fixture_path for task in tasks}
    if len(fixture_ids) != 1 or len(fixture_versions) != 1 or len(fixture_paths) != 1:
        raise ValueError("ForgeBench cases must share one fixture identity and path")
    fixture_id = next(iter(fixture_ids))
    fixture_version = next(iter(fixture_versions))
    fixture_path = next(iter(fixture_paths))
    if fixture_id is None or fixture_version is None or fixture_path is None:
        raise ValueError("ForgeBench fixture identity is incomplete")
    return {
        "fixture_id": fixture_id,
        "fixture_version": fixture_version,
        "canonical_tree_sha256": canonical_tree_digest(Path(fixture_path)),
    }


def _assert_catalog_uses_fixture(
    tasks: list[EvaluationTask],
    fixture: dict[str, str],
) -> None:
    actual = _fixture_identity_from_tasks(tasks)
    if actual != fixture:
        raise ValueError("fixture lock does not match ForgeBench catalog fixture")


def _dataset_lock_payload(
    tasks: list[EvaluationTask],
    fixture: dict[str, str],
) -> dict[str, Any]:
    development = sorted(
        (task for task in tasks if task.dataset_split == DatasetSplit.DEVELOPMENT),
        key=lambda task: task.id,
    )
    holdout = sorted(
        (task for task in tasks if task.dataset_split == DatasetSplit.HOLDOUT),
        key=lambda task: task.id,
    )
    return {
        "schema_version": 1,
        "fixture": fixture,
        "development": {
            "count": len(development),
            "ids": [task.id for task in development],
            "fingerprint": dataset_fingerprint(development),
        },
        "holdout": {
            "count": len(holdout),
            "ids": [task.id for task in holdout],
            "fingerprint": dataset_fingerprint(holdout),
        },
        "combined_fingerprint": dataset_fingerprint([*development, *holdout]),
        "verifier_tree_sha256": canonical_tree_digest(VERIFIER_ROOT),
    }


def _catalog_summary(payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "development_count": payload["development"]["count"],
        "development_fingerprint": payload["development"]["fingerprint"],
        "holdout_count": payload["holdout"]["count"],
        "holdout_fingerprint": payload["holdout"]["fingerprint"],
        "combined_fingerprint": payload["combined_fingerprint"],
        "verifier_tree_sha256": payload["verifier_tree_sha256"],
    }


def _assert_fixture_is_clean(fixture: Path) -> None:
    if not fixture.is_dir():
        raise ValueError(f"ForgeBench fixture does not exist: {fixture}")
    generated = sorted(
        path.relative_to(fixture).as_posix()
        for path in fixture.rglob("*")
        if path.is_dir() and path.name in IGNORED_TREE_PARTS
    )
    if generated:
        raise ValueError(f"ForgeBench fixture contains generated state: {generated}")


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as temporary:
            json.dump(payload, temporary, indent=2, sort_keys=True)
            temporary.write("\n")
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
    except BaseException:
        Path(temporary_name).unlink(missing_ok=True)
        raise


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m app.forgebench")
    subcommands = parser.add_subparsers(dest="command", required=True)
    freeze = subcommands.add_parser("freeze-fixture")
    freeze.add_argument("--fixture", type=Path, required=True)
    freeze.add_argument("--output", type=Path, required=True)
    freeze_dataset_parser = subcommands.add_parser("freeze-dataset")
    freeze_dataset_parser.add_argument("--cases", type=Path, required=True)
    freeze_dataset_parser.add_argument("--fixture-lock", type=Path, required=True)
    freeze_dataset_parser.add_argument("--output", type=Path, required=True)
    verify = subcommands.add_parser("verify-catalog")
    verify.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    verify.add_argument("--lock", type=Path, default=DEFAULT_DATASET_LOCK)
    show = subcommands.add_parser("show-lock")
    show.add_argument("--lock", type=Path, default=DEFAULT_DATASET_LOCK)
    show.add_argument("--split", type=DatasetSplit, choices=list(DatasetSplit), required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "freeze-fixture":
        payload = freeze_fixture(args.fixture, args.output)
        print(
            json.dumps(
                {
                    "fixture_id": payload["fixture_id"],
                    "fixture_version": payload["fixture_version"],
                    "canonical_tree_sha256": payload["canonical_tree_sha256"],
                },
                sort_keys=True,
            )
        )
        return 0
    if args.command == "freeze-dataset":
        payload = freeze_dataset(args.cases, args.fixture_lock, args.output)
        print(json.dumps(_catalog_summary(payload), sort_keys=True))
        return 0
    if args.command == "verify-catalog":
        print(json.dumps(verify_catalog(args.cases, args.lock), sort_keys=True))
        return 0
    if args.command == "show-lock":
        print(json.dumps(show_lock(args.lock, args.split), sort_keys=True))
        return 0
    raise AssertionError(f"unhandled ForgeBench command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
