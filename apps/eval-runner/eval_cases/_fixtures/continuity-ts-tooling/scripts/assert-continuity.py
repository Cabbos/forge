#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sqlite3
import sys
from collections import Counter
from pathlib import Path
from typing import Any

DEFAULT_DIRTY_TERMS = [
    "我希望你",
    "我们现在在",
    "随便问",
    "给我一段提示词",
    "完整照抄",
    "本轮提示词",
    "用户偏好：我希望你",
]
SUCCESS_MARKERS = [
    "EXIT CODE: 0",
    "EXIT: 0",
    "TSC_EXIT: 0",
    "TEST_EXIT: 0",
    "0 failed",
    "0 失败",
    "passed",
    "通过",
]
FILE_PATH_RE = re.compile(
    r"(?:^|[\s\[,;])((?:[\w.-]+/)*[\w.-]+\.(?:ts|tsx|js|jsx|json|rs|py|md|css|html))"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Assert Forge Continuity SQLite health.")
    parser.add_argument("--db", default=".forge/continuity.db")
    parser.add_argument("--min-experiences", type=int, default=0)
    parser.add_argument("--max-dirty-candidates", type=int)
    parser.add_argument("--max-evidence-duplicates", type=int)
    parser.add_argument("--require-formed-reflections", action="store_true")
    parser.add_argument("--require-fts-match", action="store_true")
    parser.add_argument("--require-shell-success-clean", action="store_true")
    parser.add_argument("--require-event", action="append", default=[])
    parser.add_argument("--require-experience-text", action="append", default=[])
    parser.add_argument("--dirty-term", action="append", default=[])
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    db_path = Path(args.db)
    errors: list[str] = []
    summary: dict[str, Any] = {
        "ok": False,
        "db": str(db_path),
        "errors": errors,
    }

    if not db_path.exists():
        errors.append(f"continuity db does not exist: {db_path}")
        print(json.dumps(summary, ensure_ascii=False, indent=2))
        return 1

    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        event_rows = load_events(conn)
        experiences = load_experiences(conn)
        event_counts = Counter(row["event_type"] for row in event_rows)
        reflection_count = event_counts.get("reflection", 0)
        formed_reflection_count = table_count(conn, "continuity_formed_reflections")
        fts_count = table_count(conn, "continuity_experiences_fts")
    except sqlite3.Error as exc:
        errors.append(f"sqlite query failed: {exc}")
        print(json.dumps(summary, ensure_ascii=False, indent=2))
        return 1
    finally:
        if "conn" in locals():
            conn.close()

    dirty_terms = [*DEFAULT_DIRTY_TERMS, *args.dirty_term]
    dirty_candidates = dirty_experiences(experiences, dirty_terms)
    shell_false_errors = shell_success_false_errors(event_rows)
    evidence_duplicates = duplicated_evidence_path_count(experiences)

    summary.update(
        {
            "event_counts": dict(sorted(event_counts.items())),
            "experience_count": len(experiences),
            "reflection_count": reflection_count,
            "formed_reflection_count": formed_reflection_count,
            "fts_count": fts_count,
            "dirty_candidate_count": len(dirty_candidates),
            "shell_success_false_error_count": shell_false_errors,
            "evidence_duplicate_count": evidence_duplicates,
        }
    )

    if len(experiences) < args.min_experiences:
        errors.append(
            f"experience count {len(experiences)} is below minimum {args.min_experiences}"
        )
    for event_type in args.require_event:
        if event_counts.get(event_type, 0) == 0:
            errors.append(f"required event type is missing: {event_type}")
    if args.require_formed_reflections and formed_reflection_count != reflection_count:
        errors.append(
            "formed reflection count does not match reflection count: "
            f"{formed_reflection_count} != {reflection_count}"
        )
    if args.require_fts_match and fts_count != len(experiences):
        errors.append(
            f"FTS row count does not match experiences: {fts_count} != {len(experiences)}"
        )
    if args.max_dirty_candidates is not None and len(dirty_candidates) > args.max_dirty_candidates:
        errors.append(
            f"dirty candidate count {len(dirty_candidates)} exceeds max {args.max_dirty_candidates}"
        )
    for required_text in args.require_experience_text:
        if not experience_text_exists(experiences, required_text):
            errors.append(f"required experience text is missing: {required_text}")
    if args.require_shell_success_clean and shell_false_errors > 0:
        errors.append(f"successful shell output recorded as error: {shell_false_errors}")
    if (
        args.max_evidence_duplicates is not None
        and evidence_duplicates > args.max_evidence_duplicates
    ):
        errors.append(
            f"evidence duplicate count {evidence_duplicates} exceeds max "
            f"{args.max_evidence_duplicates}"
        )

    summary["ok"] = len(errors) == 0
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0 if summary["ok"] else 1


def load_events(conn: sqlite3.Connection) -> list[sqlite3.Row]:
    return conn.execute(
        "SELECT event_type, event_json FROM continuity_events ORDER BY timestamp_ms, id"
    ).fetchall()


def load_experiences(conn: sqlite3.Connection) -> list[sqlite3.Row]:
    return conn.execute(
        "SELECT id, kind, status, title, body "
        "FROM continuity_experiences "
        "ORDER BY created_at_ms, id"
    ).fetchall()


def table_count(conn: sqlite3.Connection, table_name: str) -> int:
    exists = conn.execute(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?",
        (table_name,),
    ).fetchone()[0]
    if not exists:
        return 0
    return int(conn.execute(f"SELECT COUNT(*) FROM {table_name}").fetchone()[0])


def dirty_experiences(experiences: list[sqlite3.Row], dirty_terms: list[str]) -> list[str]:
    dirty: list[str] = []
    for experience in experiences:
        text = f"{experience['title']}\n{experience['body']}"
        if any(term and term in text for term in dirty_terms):
            dirty.append(str(experience["id"]))
    return dirty


def shell_success_false_errors(event_rows: list[sqlite3.Row]) -> int:
    false_errors = 0
    for row in event_rows:
        if row["event_type"] != "tool_execution":
            continue
        event = unwrap_event_json(row["event_json"], row["event_type"])
        if not bool(event.get("is_error")):
            continue
        text = " ".join(
            str(event.get(key, "")) for key in ["tool_name", "input_summary", "output_summary"]
        )
        if any(marker in text for marker in SUCCESS_MARKERS):
            false_errors += 1
    return false_errors


def unwrap_event_json(raw_json: str, event_type: str) -> dict[str, Any]:
    try:
        payload = json.loads(raw_json)
    except json.JSONDecodeError:
        return {}
    if isinstance(payload, dict) and isinstance(payload.get(event_type), dict):
        return payload[event_type]
    if isinstance(payload, dict) and len(payload) == 1:
        only_value = next(iter(payload.values()))
        if isinstance(only_value, dict):
            return only_value
    return payload if isinstance(payload, dict) else {}


def experience_text_exists(experiences: list[sqlite3.Row], needle: str) -> bool:
    needle = needle.casefold()
    return any(
        needle in f"{experience['title']}\n{experience['body']}".casefold()
        for experience in experiences
    )


def duplicated_evidence_path_count(experiences: list[sqlite3.Row]) -> int:
    duplicate_count = 0
    for experience in experiences:
        evidence_text = evidence_text_from_body(experience["body"] or "")
        paths = [match.group(1) for match in FILE_PATH_RE.finditer(evidence_text)]
        counts = Counter(paths)
        duplicate_count += sum(count - 1 for count in counts.values() if count > 1)
    return duplicate_count


def evidence_text_from_body(body: str) -> str:
    if "Evidence:" in body:
        return body.split("Evidence:", 1)[1]
    return "\n".join(line for line in body.splitlines() if "file_changes=" in line)


if __name__ == "__main__":
    sys.exit(main())
