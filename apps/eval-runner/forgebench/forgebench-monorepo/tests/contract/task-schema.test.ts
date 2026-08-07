import { expect, test } from "bun:test";
import {
  isTaskPriority,
  isTaskStatus,
  TASK_PRIORITIES,
  TASK_STATUSES,
} from "../../packages/domain/src";

test("task status and priority contracts are explicit", () => {
  expect(TASK_STATUSES).toEqual(["todo", "in_progress", "done", "cancelled"]);
  expect(TASK_PRIORITIES).toEqual(["low", "medium", "high", "urgent"]);
  expect(isTaskStatus("done")).toBe(true);
  expect(isTaskStatus("archived")).toBe(false);
  expect(isTaskPriority("urgent")).toBe(true);
});
