import { expect, test } from "bun:test";
import { optimisticTaskUpdate } from "../../apps/web-console/src/task-list";

test("failed optimistic update restores input task", async () => {
  const task = { id: "1", title: "Before", status: "todo", priority: "low", revision: 1 };
  await expect(
    optimisticTaskUpdate(task, { title: "After" }, async () => {
      throw new Error("network");
    }),
  ).rejects.toThrow();
  expect(task.title).toBe("Before");
});
