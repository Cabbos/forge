import { expect, test } from "bun:test";
import { normalizeDueDate } from "../../packages/domain/src/task";

test("due date keeps its represented instant", () => {
  expect(normalizeDueDate("2026-08-07T10:30:00+08:00")).toBe("2026-08-07T02:30:00.000Z");
});
