from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const domain = await import(`${root}/packages/domain/src/task-query.ts`);
test("keeps quoted search text with a status token", () => {
  const query = domain.parseTaskQuery('status:in_progress "release train"');
  expect(query).toEqual({ status: "in_progress", search: "release train" });
  expect(
    domain.matchesTaskQuery(
      { id: "1", title: "Release train", status: "in_progress", priority: "high", revision: 1 },
      query,
    ),
  ).toBe(true);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-web-filter-query", check)
