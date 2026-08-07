from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { shouldHandleShortcut } = await import(`${root}/apps/web-console/src/task-list.tsx`);
test("editable targets never trigger task shortcuts", () => {
  for (const tag of ["input", "INPUT", "TextArea", "select"]) {
    expect(shouldHandleShortcut(tag, false)).toBe(false);
  }
  expect(shouldHandleShortcut("div", true)).toBe(false);
  expect(shouldHandleShortcut("button", false)).toBe(true);
  expect(shouldHandleShortcut("div", false)).toBe(true);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h12", check)
