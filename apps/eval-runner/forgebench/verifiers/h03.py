from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const filters = await import(`${root}/apps/web-console/src/saved-filters.ts`);
test("serialization is canonical and does not invent optionals", () => {
  const input = [
    { name: " Zed ", query: { status: "done" } },
    { name: "Alpha", query: { search: "release" } },
  ];
  const encoded = filters.serializeSavedFilters(input);
  expect(encoded).toBe(
    '[{"name":"Alpha","query":{"search":"release"}},{"name":"Zed","query":{"status":"done"}}]',
  );
  expect(filters.serializeSavedFilters([...input].reverse())).toBe(encoded);
  expect(filters.deserializeSavedFilters(encoded)).toEqual([
    { name: "Alpha", query: { search: "release" } },
    { name: "Zed", query: { status: "done" } },
  ]);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h03", check)
