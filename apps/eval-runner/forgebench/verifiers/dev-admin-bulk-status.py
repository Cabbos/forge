from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { bulkTransition } = await import(`${root}/apps/admin-console/src/bulk-actions.ts`);
test("does not partially mutate on a late invalid transition", () => {
  const tasks = [
    { id: "1", title: "First", status: "todo", priority: "low", revision: 2 },
    { id: "2", title: "Second", status: "cancelled", priority: "high", revision: 7 },
  ];
  const before = structuredClone(tasks);
  expect(() => bulkTransition(tasks, "in_progress", "admin")).toThrow();
  expect(tasks).toEqual(before);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-admin-bulk-status", check)
