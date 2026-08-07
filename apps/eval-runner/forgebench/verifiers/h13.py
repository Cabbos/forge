from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { parsePort } = await import(`${root}/packages/config/src/env.ts`);
test("accepts only canonical integer ports in the TCP interval", () => {
  expect(parsePort("1")).toBe(1);
  expect(parsePort("65535")).toBe(65535);
  expect(parsePort(undefined, 9876)).toBe(9876);
  for (const value of ["0", "65536", "-1", "+80", " 80", "80 ", "1.5", "1e3", ""]) {
    expect(() => parsePort(value)).toThrow();
  }
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h13", check)
