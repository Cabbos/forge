from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { parsePort } = await import(`${root}/packages/config/src/env.ts`);
test("accepts only canonical TCP port integers", () => {
  for (const value of ["", " ", "+80", "08x", "1.5", "0", "65536", "9007199254740992"]) {
    expect(() => parsePort(value)).toThrow();
  }
  expect(parsePort("1")).toBe(1);
  expect(parsePort("65535")).toBe(65535);
  expect(parsePort(undefined, 4321)).toBe(4321);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-config-env-parser", check)
