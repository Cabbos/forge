import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  deleteProviderProfile,
  getApiKeyStatus,
  getProviderCatalog,
  listProviderModels,
  probeProvider,
  upsertProviderProfile,
} from "./apiKeys.ts";

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

describe("listProviderModels", () => {
  it("returns a clear error outside the Tauri runtime", async () => {
    await assert.rejects(
      listProviderModels("openai"),
      /Provider model catalog is not available outside Tauri runtime/,
    );
  });
});

describe("provider profile editing", () => {
  it("returns clear errors outside the Tauri runtime", async () => {
    await assert.rejects(
      upsertProviderProfile({
        id: "local-openai",
        label: "Local OpenAI",
        transport: "openai_chat_completions",
        base_url: "http://127.0.0.1:1234/v1",
        api_key_env: [],
        base_url_env: [],
        default_model: "local-model",
        aliases: [],
        supports_tools: true,
        supports_streaming: true,
      }),
      /Provider profile editing is not available outside Tauri runtime/,
    );
    await assert.rejects(
      deleteProviderProfile("local-openai"),
      /Provider profile editing is not available outside Tauri runtime/,
    );
  });
});
