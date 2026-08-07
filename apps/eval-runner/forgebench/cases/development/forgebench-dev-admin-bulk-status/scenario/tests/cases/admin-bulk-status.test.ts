import { expect, test } from "bun:test";
import { bulkTransition } from "../../apps/admin-console/src/bulk-actions";
import type { Task } from "../../packages/domain/src";

test("invalid bulk transition leaves every input untouched", () => {
  const tasks: Task[] = [
    { id: "1", title: "Todo", status: "todo", priority: "low", revision: 1 },
    { id: "2", title: "Done", status: "done", priority: "high", revision: 4 },
  ];
  expect(() => bulkTransition(tasks, "in_progress", "admin")).toThrow();
  expect(tasks.map(({ status, revision }) => ({ status, revision }))).toEqual([
    { status: "todo", revision: 1 },
    { status: "done", revision: 4 },
  ]);
});
