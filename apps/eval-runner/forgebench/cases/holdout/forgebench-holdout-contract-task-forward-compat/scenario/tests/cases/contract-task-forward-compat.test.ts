import { expect, test } from "bun:test";
import { decodeTask } from "../../packages/domain/src/task";

test("task decoder ignores an unknown optional producer field", () => {
  expect(
    decodeTask({
      id: "task-1",
      title: "Ship",
      status: "todo",
      priority: "high",
      revision: 1,
      futureLabel: "from-v2",
    }).title,
  ).toBe("Ship");
});
