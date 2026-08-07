from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { decodeTask } = await import(`${root}/packages/domain/src/task.ts`);
const { decodeTaskRow } = await import(`${root}/apps/web-console/src/task-list.tsx`);
test("unknown optional fields are ignored while known fields remain validated", () => {
  const future = {
    id: "one",
    title: "Forward compatible",
    status: "todo",
    priority: "high",
    revision: 2,
    producerTrace: { region: "east" },
  };
  const decoded = decodeTask(future);
  expect(decoded).toEqual({
    id: "one",
    title: "Forward compatible",
    status: "todo",
    priority: "high",
    revision: 2,
  });
  expect(decodeTaskRow(future).id).toBe("one");
  expect(() => decodeTask({ ...future, status: "paused" })).toThrow();
  expect(() => decodeTask({ ...future, revision: 0 })).toThrow();
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h17", check)
