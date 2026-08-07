import { expect, test } from "bun:test";
import { bulkTransition } from "../../apps/admin-console/src/bulk-actions";
import type { Task } from "../../packages/domain/src";

test("admin bulk edit is authorized and atomic", () => {
  const tasks: Task[] = [
    { id: "1", title: "First", status: "todo", priority: "low", revision: 1 },
    { id: "2", title: "Second", status: "todo", priority: "high", revision: 3 },
  ];
  expect(bulkTransition(tasks, "in_progress", "operator").map((task) => task.revision)).toEqual([
    2, 4,
  ]);
  expect(() => bulkTransition(tasks, "done", "operator")).toThrow();
  expect(() => bulkTransition(tasks, "in_progress", "viewer")).toThrow();
  expect(tasks.every((task) => task.status === "todo")).toBe(true);
});
