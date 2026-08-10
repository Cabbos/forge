from pathlib import Path
from typing import Any

from _support import run_typescript_test, verify


def check(workspace: Path) -> dict[str, Any]:
    source = r"""
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
const root = process.env.FORGEBENCH_WORKSPACE!;
const { sha256File } = await import(`${root}/tools/release/checksums.ts`);
test("hashes bytes deterministically with SHA-256", async () => {
  const directory = await mkdtemp(join(tmpdir(), "forgebench-checksum-"));
  const left = join(directory, "left.bin");
  const right = join(directory, "right.bin");
  await writeFile(left, Buffer.from([0, 1, 2, 3]));
  await writeFile(right, Buffer.from([3, 2, 1, 0]));
  const expected = createHash("sha256").update(Buffer.from([0, 1, 2, 3])).digest("hex");
  expect(await sha256File(left)).toBe(expected);
  expect(await sha256File(left)).toBe(expected);
  expect(await sha256File(right)).not.toBe(expected);
});
"""
    return {"typescript": run_typescript_test(workspace, source)}


verify("h16", check)
