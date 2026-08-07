import { expect, test } from "bun:test";
import { taskList } from "../../apps/web-console/src/task-list";

test("task row badge uses unread notifications", () => {
  const result = taskList(
    [
      {
        id: "task-1",
        title: "Ship",
        status: "todo",
        priority: "high",
        revision: 9,
        unreadNotifications: 2,
      },
    ],
    "",
  );
  expect(result.rows[0].unread).toBe(2);
});
