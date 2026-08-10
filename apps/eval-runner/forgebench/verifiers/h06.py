from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const permissions = await import(`${root}/apps/admin-console/src/permissions.ts`);
const actions = await import(`${root}/apps/admin-console/src/bulk-actions.ts`);
const task = { id: "1", title: "One", status: "todo", priority: "low", revision: 1 };
test("viewer cannot mutate while privileged roles retain access", () => {
  expect(permissions.canBulkEdit("viewer")).toBe(false);
  expect(permissions.canBulkEdit("operator")).toBe(true);
  expect(permissions.canBulkEdit("admin")).toBe(true);
  expect(() => actions.bulkTransition([task], "in_progress", "viewer")).toThrow(/authorized/u);
  expect(actions.bulkTransition([task], "in_progress", "operator")[0].status).toBe("in_progress");
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h06", check)
