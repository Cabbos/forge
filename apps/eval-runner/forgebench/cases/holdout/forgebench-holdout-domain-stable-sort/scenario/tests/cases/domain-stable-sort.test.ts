import { expect, test } from "bun:test";
import { sortTasks, type Task } from "../../packages/domain/src";

test("equal sort keys retain input order", () => {
  const tasks: Task[] = ["a", "c", "b"].map((id) => ({
    id,
    title: "Same",
    status: "todo",
    priority: "medium",
    revision: 1,
  }));
  expect(sortTasks(tasks).map((task) => task.id)).toEqual(["a", "c", "b"]);
});
