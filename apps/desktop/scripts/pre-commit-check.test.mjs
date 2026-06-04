import assert from "node:assert/strict";
import test from "node:test";

import {
  buildPreCommitPlan,
  describeCommand,
} from "./pre-commit-check.mjs";

test("blocks generated artifacts from being committed", () => {
  const plan = buildPreCommitPlan([
    "test-results/latest/report.json",
    "src/store/index.ts",
  ]);

  assert.deepEqual(plan.blockedFiles, ["test-results/latest/report.json"]);
});

test("plans frontend checks for staged TypeScript or style files", () => {
  const plan = buildPreCommitPlan([
    "src/components/session/ComposerToolbar.tsx",
    "src/styles/composer.css",
  ]);

  assert.deepEqual(plan.commands.map(describeCommand), [
    "npm run check:conversation-style",
    "npx tsc --noEmit",
  ]);
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
    "npx tsc --noEmit",
    "cargo fmt --manifest-path src-tauri/Cargo.toml --check",
    "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
  ]);
});
