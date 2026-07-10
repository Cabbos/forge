import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const desktopDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("desktop production CSS is warning-free and retains required utilities", () => {
  const result = spawnSync("npm", ["run", "build"], {
    cwd: desktopDir,
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1", FORCE_COLOR: "0" },
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;

  assert.equal(result.status, 0, output);
  assert.doesNotMatch(
    output,
    /Unknown at rule|@theme|@utility|@custom-variant|\[vite:css\]\[postcss\]|didn.t resolve at build time|\[PLUGIN_TIMINGS\]/i,
  );

  const assetsDir = join(desktopDir, "dist", "assets");
  const css = readdirSync(assetsDir)
    .filter((name) => name.endsWith(".css"))
    .map((name) => readFileSync(join(assetsDir, name), "utf8"))
    .join("\n");
  for (const requiredClass of [
    "animate-in",
    "fade-in-0",
    "zoom-in-95",
    "slide-in-from-top-2",
    "forge-app-shell",
    "bg-popover",
    "text-muted-foreground",
  ]) {
    assert.match(css, new RegExp(requiredClass), `generated CSS should contain ${requiredClass}`);
  }
});
