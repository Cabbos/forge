import argparse
import json
import os
import tempfile
from pathlib import Path
from typing import Any

from app.artifact_hashing import IGNORED_TREE_PARTS, canonical_tree_digest

FIXTURE_ID = "forgebench-monorepo"
FIXTURE_VERSION = "1.0.0"
FIXTURE_VERIFICATION_COMMAND = "python3 tools/release/verify.py"


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
    raise AssertionError(f"unhandled ForgeBench command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
