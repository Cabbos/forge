from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { normalizeDueDate } = await import(`${root}/packages/domain/src/task.ts`);
test("preserves instants and rejects impossible dates", () => {
  expect(normalizeDueDate("2026-08-07T10:30:00+08:00")).toBe("2026-08-07T02:30:00.000Z");
  expect(normalizeDueDate("2026-08-06T21:30:00-05:00")).toBe("2026-08-07T02:30:00.000Z");
  expect(() => normalizeDueDate("2026-02-30T10:00:00Z")).toThrow();
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h01", check)
