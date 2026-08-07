import { expect, test } from "bun:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildManifest } from "../../tools/release/build-manifest";

test("artifact manifest is sorted and uses SHA-256", async () => {
  const root = await mkdtemp(join(tmpdir(), "forgebench-manifest-"));
  await writeFile(join(root, "z.txt"), "z\n");
  await writeFile(join(root, "a.txt"), "a\n");
  const manifest = await buildManifest(root);
  expect(manifest.map((entry) => entry.path)).toEqual(["a.txt", "z.txt"]);
  expect(manifest.every((entry) => /^[a-f0-9]{64}$/u.test(entry.sha256))).toBe(true);
});
