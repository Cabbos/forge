from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { priorityBadge } = await import(`${root}/packages/ui/src/priority-badge.tsx`);
test("maps every priority to label and modifier class", () => {
  const labels = { low: "Low", medium: "Medium", high: "High", urgent: "Urgent" };
  for (const [priority, label] of Object.entries(labels)) {
    const badge = priorityBadge(priority);
    expect(badge.ariaLabel).toBe(`${label} priority`);
    expect(badge.className).toContain(`priority-badge--${priority}`);
    expect(badge.text).toBe(priority);
  }
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-ui-priority-badge", check)
