import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  exportSessionStore,
  getSessionStoreStats,
  pruneSessionStore,
  searchSessionStore,
} from "./sessionStore.ts";

describe("session store IPC fallbacks", () => {
  it("returns empty stats outside the Tauri runtime", async () => {
    const stats = await getSessionStoreStats();

    assert.equal(stats.total_snapshots, 0);
    assert.equal(stats.corrupted_snapshots, 0);
  });

  it("returns no search results outside the Tauri runtime", async () => {
    assert.deepEqual(await searchSessionStore("launch"), []);
  });

  it("throws clear errors for export and prune outside Tauri", async () => {
    await assert.rejects(
      exportSessionStore(),
      /Session store export is not available outside Tauri runtime/,
    );
    await assert.rejects(
      pruneSessionStore({ keepRecent: 25 }),
      /Session store prune is not available outside Tauri runtime/,
    );
  });
});
