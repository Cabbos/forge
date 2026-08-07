from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { normalizeTaskTitle } = await import(`${root}/packages/domain/src/task-title.ts`);
test("normalizes mixed Unicode whitespace globally", () => {
  expect(normalizeTaskTitle("\u3000Plan\u00a0\u2003\u2003release\t\nnow\u3000")).toBe(
    "Plan release now",
  );
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-domain-normalize-title", check)
