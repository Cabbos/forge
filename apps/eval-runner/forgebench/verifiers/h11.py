from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { escapeHtml } = await import(`${root}/packages/ui/src/empty-state.tsx`);
test("escapes every HTML-significant character and preserves Unicode", () => {
  expect(escapeHtml(`<&"'安全`)).toBe("&lt;&amp;&quot;&#39;安全");
  expect(escapeHtml("普通标签")).toBe("普通标签");
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h11", check)
