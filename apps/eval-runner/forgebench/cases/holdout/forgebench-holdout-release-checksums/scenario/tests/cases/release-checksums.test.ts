import { expect, test } from "bun:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { sha256File } from "../../tools/release/checksums";

test("checksums distinguish equal-length file contents", async () => {
  const directory = await mkdtemp(join(tmpdir(), "checksum-visible-"));
  const left = join(directory, "left.txt");
  const right = join(directory, "right.txt");
  await writeFile(left, "alpha");
  await writeFile(right, "bravo");
  expect(await sha256File(left)).not.toBe(await sha256File(right));
});
