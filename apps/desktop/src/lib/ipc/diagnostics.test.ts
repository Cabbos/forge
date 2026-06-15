import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  attachGatewaySession,
  enqueueGatewayTrigger,
  getGatewayTriggerRun,
  replayGatewayTriggerRun,
} from "./diagnostics.ts";

describe("enqueueGatewayTrigger", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      enqueueGatewayTrigger({ message: "run diagnostics smoke" }),
      /Gateway trigger enqueue is not available outside Tauri runtime/,
    );
  });
});

describe("replayGatewayTriggerRun", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      replayGatewayTriggerRun("run-1"),
      /Gateway trigger replay is not available outside Tauri runtime/,
    );
  });
});

describe("getGatewayTriggerRun", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      getGatewayTriggerRun("run-1"),
      /Gateway trigger run detail is not available outside Tauri runtime/,
    );
  });
});

describe("attachGatewaySession", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      attachGatewaySession("session-1"),
      /Gateway session attach is not available outside Tauri runtime/,
    );
  });
});
