import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

test("store keeps shared types and persistence helpers outside the root module", () => {
  assert.equal(existsSync(join(root, "src/store/types.ts")), true);
  assert.equal(existsSync(join(root, "src/store/persistence.ts")), true);
  assert.equal(existsSync(join(root, "src/store/blocks.ts")), true);
  assert.equal(existsSync(join(root, "src/store/session-utils.ts")), true);
  assert.equal(existsSync(join(root, "src/store/selectors.ts")), true);
  assert.equal(existsSync(join(root, "src/store/event-dispatch.ts")), true);
  assert.equal(existsSync(join(root, "src/store/hydration.ts")), true);
  assert.equal(existsSync(join(root, "src/store/workspace-actions.ts")), true);
  assert.equal(existsSync(join(root, "src/store/session-actions.ts")), true);
  assert.equal(existsSync(join(root, "src/store/context-actions.ts")), true);
  assert.equal(existsSync(join(root, "src/store/preferences-actions.ts")), true);

  const rootStore = read("src/store/index.ts");
  const preferencesActions = read("src/store/preferences-actions.ts");
  assert.match(rootStore, /from "\.\/types"/);
  assert.match(preferencesActions, /from "\.\/persistence"/);
  assert.match(rootStore, /from "\.\/selectors"/);
  assert.match(rootStore, /from "\.\/event-dispatch"/);
  assert.match(rootStore, /from "\.\/hydration"/);
  assert.match(rootStore, /from "\.\/workspace-actions"/);
  assert.match(rootStore, /from "\.\/session-actions"/);
  assert.match(rootStore, /from "\.\/context-actions"/);
  assert.match(rootStore, /from "\.\/preferences-actions"/);
  assert.match(rootStore, /dispatchOutputEvent: createOutputEventDispatcher\(set, get\)/);
  assert.match(rootStore, /hydrate: createHydrateAction\(set, get\)/);
  assert.match(rootStore, /\.\.\.createWorkspaceActions\(set, get\)/);
  assert.match(rootStore, /\.\.\.createContextActions\(set, get\)/);
  assert.match(rootStore, /\.\.\.createSessionActions\(set, get\)/);
  assert.match(rootStore, /\.\.\.createPreferencesActions\(set, get\)/);
  assert.doesNotMatch(rootStore, /const PERSIST_KEY =/);
  assert.doesNotMatch(rootStore, /function persistSessions\(/);
  assert.doesNotMatch(rootStore, /function loadBlocks\(/);
  assert.doesNotMatch(rootStore, /function eventToBlock\(/);
  assert.doesNotMatch(rootStore, /function transcriptEventsToBlocks\(/);
  assert.doesNotMatch(rootStore, /function buildContextUsage\(/);
  assert.doesNotMatch(rootStore, /function workspaceSessionIds\(/);
  assert.doesNotMatch(rootStore, /function sortSessionsByRecency\(/);
  assert.doesNotMatch(rootStore, /export const useActiveSession = \(\) =>/);
  assert.doesNotMatch(rootStore, /if \(event_type === "workflow_updated"\)/);
  assert.doesNotMatch(rootStore, /const chunkTypes =/);
  assert.doesNotMatch(rootStore, /const backendMetadata =/);
  assert.doesNotMatch(rootStore, /await loadBlocks\(/);
  assert.doesNotMatch(rootStore, /setActiveWorkspace: \(id\) =>/);
  assert.doesNotMatch(rootStore, /upsertWorkspace: \(workspace\) =>/);
  assert.doesNotMatch(rootStore, /addSession: \(id, provider, model, workingDir\) =>/);
  assert.doesNotMatch(rootStore, /removeSession: \(id\) =>/);
  assert.doesNotMatch(rootStore, /upsertMemory: \(memory\) =>/);
  assert.doesNotMatch(rootStore, /setSelectedProvider: \(p\) =>/);
  assert.doesNotMatch(rootStore, /setSelectedModel: \(m\) =>/);
  assert.doesNotMatch(rootStore, /setTheme: \(theme\) =>/);
});
