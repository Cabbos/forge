import { expect, test } from "bun:test";
import { priorityBadge } from "../../packages/ui/src/priority-badge";

test("priority badge exposes its exact priority", () => {
  expect(priorityBadge("high")).toEqual({
    ariaLabel: "High priority",
    className: "priority-badge priority-badge--high",
    text: "high",
  });
});
