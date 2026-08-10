from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { buildManifest } = await import(`${root}/tools/release/build-manifest.ts`);
test("is deterministic and cryptographic", async () => {
  const sample = await mkdtemp(join(tmpdir(), "manifest-hidden-"));
  await writeFile(join(sample, "z.txt"), "same length\n");
  await writeFile(join(sample, "a.txt"), "different!!\n");
  const first = await buildManifest(sample);
  const second = await buildManifest(sample);
  expect(first).toEqual(second);
  expect(first.map((entry) => entry.path)).toEqual(["a.txt", "z.txt"]);
  expect(first.every((entry) => /^[a-f0-9]{64}$/u.test(entry.sha256))).toBe(true);
  expect(first[0].sha256).not.toBe(first[1].sha256);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("dev-release-artifact-manifest", check)
