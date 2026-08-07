import { expect, test } from "bun:test";
import { shouldHandleShortcut } from "../../apps/web-console/src/task-list";

test("shortcuts ignore lowercase editable controls", () => {
  expect(shouldHandleShortcut("input", false)).toBe(false);
  expect(shouldHandleShortcut("div", true)).toBe(false);
  expect(shouldHandleShortcut("button", false)).toBe(true);
});
