from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { sortTasks } = await import(`${root}/packages/domain/src/task-sort.ts`);
test("stable ties preserve caller order without mutating input", () => {
  const tasks = ["z", "a", "m"].map((id) => ({
    id, title: "Same", status: "todo", priority: "medium", revision: 1,
  }));
  const before = structuredClone(tasks);
  expect(sortTasks(tasks).map((task) => task.id)).toEqual(["z", "a", "m"]);
  expect(tasks).toEqual(before);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h02", check)
