import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  const absolutePath = join(root, path);
  assert.equal(existsSync(absolutePath), true, `${path} should exist`);
  return readFileSync(absolutePath, "utf8");
}

test("CI workflow covers the monorepo quality gates", () => {
  const workflow = read(".github/workflows/ci.yml");
  const rootPackage = JSON.parse(read("package.json"));

  assert.match(workflow, /pull_request:/);
  assert.match(workflow, /push:/);
  assert.match(workflow, /branches:\s*\[\s*main\s*\]/);
  assert.match(workflow, /paths:/);
  assert.match(workflow, /apps\/desktop\/\*\*/);
  assert.match(workflow, /apps\/eval-runner\/\*\*/);
  assert.match(workflow, /apps\/website\/\*\*/);

  assert.match(workflow, /desktop-frontend:/);
  assert.match(workflow, /npm run check:frontend-architecture/);
  assert.match(workflow, /npm run build/);

  assert.match(workflow, /desktop-backend:/);
  assert.match(workflow, /desktop-backend:[\s\S]*?runs-on:\s*macos-latest/);
  assert.match(workflow, /desktop-backend:[\s\S]*?npm run build/);
  assert.match(workflow, /cargo fmt[\s\S]*?--check/);
  assert.match(workflow, /cargo clippy[\s\S]*?--all-targets -- -D warnings/);
  assert.match(workflow, /cargo test[\s\S]*?src-tauri\/Cargo.toml/);

  assert.match(workflow, /eval-runner:/);
  assert.match(workflow, /uv sync --dev/);
  assert.match(workflow, /uv run pytest/);

  assert.match(workflow, /website:/);
  assert.match(workflow, /npm run build/);

  assert.match(workflow, /nightly-eval:/);
  assert.match(workflow, /schedule:/);
  assert.match(workflow, /upload-artifact/);

  assert.equal(rootPackage.scripts["eval:report"], "npm --prefix apps/desktop run eval:report");
  assert.equal(
    rootPackage.scripts["eval:report:latest"],
    "npm --prefix apps/desktop run eval:report:latest",
  );
});

test("Desktop release workflow is manual or tag gated", () => {
  const workflow = read(".github/workflows/desktop-release.yml");

  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /tags:/);
  assert.match(workflow, /desktop-v\*/);
  assert.match(workflow, /runs-on:\s*macos-latest/);
  assert.match(workflow, /npm run tauri:build/);
  assert.match(workflow, /upload-artifact/);
});
