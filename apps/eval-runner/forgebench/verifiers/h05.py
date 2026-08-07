from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { pageForQuery } = await import(`${root}/apps/admin-console/src/bulk-actions.ts`);
test("only a changed query resets pagination", () => {
  expect(pageForQuery("status:todo", "status:todo", 9)).toBe(9);
  expect(pageForQuery("", "", 4)).toBe(4);
  expect(pageForQuery("status:todo", "status:done", 9)).toBe(1);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h05", check)
