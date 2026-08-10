from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { parseFeatureFlag } = await import(`${root}/packages/config/src/feature-flags.ts`);
test("only exact lowercase booleans are accepted", () => {
  expect(parseFeatureFlag("true")).toBe(true);
  expect(parseFeatureFlag("false")).toBe(false);
  expect(parseFeatureFlag(undefined, true)).toBe(true);
  for (const value of ["TRUE", "False", "yes", "1", " true", "false "]) {
    expect(() => parseFeatureFlag(value)).toThrow();
  }
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h14", check)
