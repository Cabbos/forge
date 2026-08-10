import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("root desktop commands distinguish full app and frontend-only startup", async () => {
  const packageJson = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );

  assert.equal(
    packageJson.scripts["dev:desktop"],
    "npm --prefix apps/desktop run tauri dev",
  );
  assert.equal(
    packageJson.scripts["dev:desktop:web"],
    "npm --prefix apps/desktop run dev",
  );
});
