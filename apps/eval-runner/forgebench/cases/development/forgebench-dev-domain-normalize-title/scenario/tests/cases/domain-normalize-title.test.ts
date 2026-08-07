import { expect, test } from "bun:test";
import { normalizeTaskTitle } from "../../packages/domain/src/task-title";

test("normalizes repeated task-title whitespace", () => {
  expect(normalizeTaskTitle("  Ship\u00a0\u00a0\u00a0release  ")).toBe("Ship release");
});
