import { describe, expect, test } from "bun:test";
import {
  canTransition,
  matchesTaskQuery,
  normalizeDueDate,
  normalizeTaskTitle,
  parseTaskQuery,
  sortTasks,
  type Task,
} from "../../packages/domain/src";

const tasks: Task[] = [
  { id: "1", title: "Write docs", status: "todo", priority: "high", revision: 1 },
  { id: "2", title: "Ship release", status: "done", priority: "urgent", revision: 2 },
];

describe("task domain", () => {
  test("normalizes titles and due dates", () => {
    expect(normalizeTaskTitle("  Ship\u00a0\u00a0release  ")).toBe("Ship release");
    expect(normalizeDueDate("2026-08-07T10:00:00+08:00")).toBe("2026-08-07T02:00:00.000Z");
  });

  test("filters, sorts, and validates transitions", () => {
    const query = parseTaskQuery('status:todo "write docs"');
    expect(tasks.filter((task) => matchesTaskQuery(task, query)).map((task) => task.id)).toEqual([
      "1",
    ]);
    expect(sortTasks(tasks).map((task) => task.id)).toEqual(["2", "1"]);
    expect(canTransition("todo", "done")).toBe(false);
    expect(canTransition("todo", "in_progress")).toBe(true);
  });
});
