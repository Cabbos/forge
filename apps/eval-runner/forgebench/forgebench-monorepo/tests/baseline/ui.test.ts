import { describe, expect, test } from "bun:test";
import { emptyState, escapeHtml, priorityBadge } from "../../packages/ui/src";

describe("shared UI", () => {
  test("builds accessible priority badges", () => {
    expect(priorityBadge("urgent")).toEqual({
      ariaLabel: "Urgent priority",
      className: "priority-badge priority-badge--urgent",
      text: "urgent",
    });
  });

  test("keeps empty-state values typed and supports explicit escaping", () => {
    expect(emptyState("No tasks", "Try again.")).toEqual({
      title: "No tasks",
      detail: "Try again.",
    });
    expect(escapeHtml('<img src=x onerror="bad">')).toBe(
      "&lt;img src=x onerror=&quot;bad&quot;&gt;",
    );
  });
});
