import assert from "node:assert/strict";
import test from "node:test";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  buildPreCommitPlan,
  describeCommand,
} from "./pre-commit-check.mjs";

test("blocks generated artifacts from being committed", () => {
  const plan = buildPreCommitPlan([
    "test-results/latest/report.json",
    "artifacts/eval-runs/latest.json",
    "src/store/index.ts",
  ]);

  assert.deepEqual(plan.blockedFiles, [
    "test-results/latest/report.json",
    "artifacts/eval-runs/latest.json",
  ]);
});

test("plans frontend checks for staged TypeScript or style files", () => {
  const plan = buildPreCommitPlan([
    "src/components/session/ComposerToolbar.tsx",
    "src/styles/composer.css",
  ]);

  assert.deepEqual(plan.commands.map(describeCommand), [
    "npm run check:conversation-style",
    "npm run check:frontend-architecture",
    "npx tsc --noEmit",
  ]);
});

test("does not trigger frontend checks for pure docs or root guidance files", () => {
  const plan = buildPreCommitPlan([
    "docs/product/forge-frontend-maintainability-plan.md",
    "AGENTS.md",
    "CLAUDE.md",
  ]);

  // docs and root guidance are not frontend code; no commands should run.
  assert.deepEqual(plan.commands, []);
});

test("plans Rust formatting and lint checks for staged backend files", () => {
  const plan = buildPreCommitPlan([
    "src-tauri/src/agent/session.rs",
    "src-tauri/Cargo.toml",
  ]);

  assert.deepEqual(plan.commands.map(describeCommand), [
    "cargo fmt --manifest-path src-tauri/Cargo.toml --check",
    "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
  ]);
});

test("keeps mixed frontend and backend plans deduplicated and ordered", () => {
  const plan = buildPreCommitPlan([
    "src/lib/protocol.ts",
    "src/components/chat/ConversationLane.tsx",
    "src-tauri/src/protocol/events.rs",
  ]);

  assert.deepEqual(plan.commands.map(describeCommand), [
    "npm run check:conversation-style",
    "npm run check:frontend-architecture",
    "npx tsc --noEmit",
    "cargo fmt --manifest-path src-tauri/Cargo.toml --check",
    "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
  ]);
});

test("normalizes monorepo-prefixed desktop paths from git hooks", () => {
  const plan = buildPreCommitPlan([
    "apps/desktop/scripts/desktop-product-boundary.test.mjs",
    "apps/desktop/src-tauri/src/lib.rs",
    "apps/desktop/artifacts/eval-runs/latest.json",
  ]);

  assert.deepEqual(plan.blockedFiles, ["artifacts/eval-runs/latest.json"]);
  assert.deepEqual(plan.commands.map(describeCommand), [
    "npm run check:conversation-style",
    "npm run check:frontend-architecture",
    "npx tsc --noEmit",
    "cargo fmt --manifest-path src-tauri/Cargo.toml --check",
    "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
  ]);
});

test("protocol sync check passes", () => {
  const output = execFileSync("npm", ["run", "check:protocol"], {
    encoding: "utf-8",
    cwd: fileURLToPath(new URL("..", import.meta.url)),
  });
  assert.match(output, /OK: all \d+ Rust StreamEvent types are handled/);
});
