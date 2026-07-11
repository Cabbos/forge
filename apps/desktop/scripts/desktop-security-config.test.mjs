import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const desktopRoot = new URL("../", import.meta.url);

async function readJson(relativePath) {
  return JSON.parse(await readFile(new URL(relativePath, desktopRoot), "utf8"));
}

const productionCsp =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

const developmentCsp =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost http://localhost:1420 http://127.0.0.1:1420 ws://localhost:1420 ws://127.0.0.1:1420; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

test("desktop security config uses explicit production and local-only development CSPs", async () => {
  const config = await readJson("src-tauri/tauri.conf.json");
  const security = config.app?.security;

  assert.equal(security?.csp, productionCsp);
  assert.equal(security?.devCsp, developmentCsp);
  assert.equal(security?.freezePrototype, true);
});

test("main-window capability is the exact runtime permission set", async () => {
  const capability = await readJson("src-tauri/capabilities/default.json");

  assert.deepEqual(capability.windows, ["main"]);
  assert.deepEqual(capability.permissions, [
    "dialog:allow-open",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
  ]);
});

test("unused shell plugin is absent from desktop frontend and backend", async () => {
  const packageJson = await readJson("package.json");
  const packageLock = await readJson("package-lock.json");
  const cargoToml = await readFile(new URL("src-tauri/Cargo.toml", desktopRoot), "utf8");
  const rustEntry = await readFile(new URL("src-tauri/src/lib.rs", desktopRoot), "utf8");

  assert.equal(packageJson.dependencies?.["@tauri-apps/plugin-shell"], undefined);
  assert.equal(packageLock.packages?.[""]?.dependencies?.["@tauri-apps/plugin-shell"], undefined);
  assert.equal(packageLock.packages?.["node_modules/@tauri-apps/plugin-shell"], undefined);
  assert.doesNotMatch(cargoToml, /tauri-plugin-shell/);
  assert.doesNotMatch(rustEntry, /tauri_plugin_shell/);
});
