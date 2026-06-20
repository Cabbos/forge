import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { getApiKeyStatus, getProviderCatalog, probeProvider } from "./apiKeys.ts";

describe("getApiKeyStatus", () => {
  it("returns an empty key list outside the Tauri runtime", async () => {
    assert.deepEqual(await getApiKeyStatus(), []);
  });
});

describe("getProviderCatalog", () => {
  it("returns an empty catalog outside the Tauri runtime", async () => {
    assert.deepEqual(await getProviderCatalog(), []);
  });
});

describe("probeProvider", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      probeProvider("openai"),
      /Provider probe is not available outside Tauri runtime/,
    );
  });
});
