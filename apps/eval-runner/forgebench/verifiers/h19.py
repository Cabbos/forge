from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { taskList } = await import(`${root}/apps/web-console/src/task-list.tsx`);
test("unread badge uses the notification count and defaults only when absent", () => {
  const tasks = [
    {
      id: "with-unread", title: "One", status: "todo", priority: "high",
      revision: 41, unreadNotifications: 3,
    },
    {
      id: "without-unread", title: "Two", status: "todo", priority: "low",
      revision: 9,
    },
  ];
  const rows = taskList(tasks, "").rows;
  expect(rows.find((row) => row.id === "with-unread")?.unread).toBe(3);
  expect(rows.find((row) => row.id === "without-unread")?.unread).toBe(0);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h19", check)
