from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { optimisticTaskUpdate } = await import(`${root}/apps/web-console/src/task-list.tsx`);
const original = { id: "1", title: "Before", status: "todo", priority: "low", revision: 3 };
test("failure preserves original and success returns an immutable candidate", async () => {
  await expect(
    optimisticTaskUpdate(original, { title: "Broken" }, async () => {
      throw new Error("network");
    }),
  ).rejects.toThrow("network");
  expect(original.title).toBe("Before");
  const result = await optimisticTaskUpdate(original, { title: "After" }, async () => {});
  expect(result.title).toBe("After");
  expect(original.title).toBe("Before");
  expect(result).not.toBe(original);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h04", check)
