import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { getApiKeyStatus } from "./apiKeys.ts";

describe("getApiKeyStatus", () => {
  it("returns an empty key list outside the Tauri runtime", async () => {
    assert.deepEqual(await getApiKeyStatus(), []);
  });
});
