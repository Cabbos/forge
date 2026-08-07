from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { assertMatchingVersions } = await import(`${root}/tools/release/version-match.ts`);
test("returns the common version and identifies mismatched manifests", () => {
  expect(
    assertMatchingVersions([
      { name: "root", version: "1.0.0" },
      { name: "task-api", version: "1.0.0" },
      { name: "event-worker", version: "1.0.0" },
    ]),
  ).toBe("1.0.0");
  expect(() =>
    assertMatchingVersions([
      { name: "root", version: "1.0.0" },
      { name: "task-api", version: "1.0.1" },
      { name: "event-worker", version: "1.0.0" },
    ]),
  ).toThrow(/task-api/u);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h15", check)
