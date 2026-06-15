import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { enqueueGatewayTrigger } from "./diagnostics.ts";

describe("enqueueGatewayTrigger", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      enqueueGatewayTrigger({ message: "run diagnostics smoke" }),
      /Gateway trigger enqueue is not available outside Tauri runtime/,
    );
  });
});
