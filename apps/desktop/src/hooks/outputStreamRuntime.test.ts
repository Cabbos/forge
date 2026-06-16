import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { shouldSubscribeToTauriOutputStream } from "./outputStreamRuntime.ts";

describe("shouldSubscribeToTauriOutputStream", () => {
  it("skips stream subscription outside the Tauri runtime", () => {
    assert.equal(shouldSubscribeToTauriOutputStream(false), false);
  });

  it("allows stream subscription inside the Tauri runtime", () => {
    assert.equal(shouldSubscribeToTauriOutputStream(true), true);
  });
});
