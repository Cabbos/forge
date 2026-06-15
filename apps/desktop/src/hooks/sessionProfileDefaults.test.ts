import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { resolveProfileSessionDefaults } from "./sessionProfileDefaults.ts";
import type { ProfileListPayload } from "../lib/ipc/types.ts";

const payload: ProfileListPayload = {
  active_profile_id: "ops",
  profiles: [
    {
      id: "default",
      name: "Default",
      default_provider: null,
      default_model: null,
      default_workspace: null,
      api_key_overrides: null,
      created_at_ms: 1,
      updated_at_ms: 1,
    },
    {
      id: "ops",
      name: "Ops",
      default_provider: "anthropic",
      default_model: "claude-sonnet-4-5",
      default_workspace: "/Users/cabbos/project/ops",
      api_key_overrides: null,
      created_at_ms: 2,
      updated_at_ms: 2,
    },
  ],
};

describe("resolveProfileSessionDefaults", () => {
  it("uses the active profile defaults for new desktop sessions", () => {
    const result = resolveProfileSessionDefaults({
      workingDir: "/Users/cabbos/project/forge",
      provider: "deepseek",
      model: "deepseek-chat",
      profiles: payload,
    });

    assert.deepEqual(result, {
      workingDir: "/Users/cabbos/project/ops",
      provider: "anthropic",
      model: "claude-sonnet-4-5",
      profileId: "ops",
    });
  });

  it("falls back to the current composer selection when no active profile exists", () => {
    const result = resolveProfileSessionDefaults({
      workingDir: "/Users/cabbos/project/forge",
      provider: "deepseek",
      model: "deepseek-chat",
      profiles: { ...payload, active_profile_id: "missing" },
    });

    assert.deepEqual(result, {
      workingDir: "/Users/cabbos/project/forge",
      provider: "deepseek",
      model: "deepseek-chat",
      profileId: null,
    });
  });

  it("ignores blank profile defaults", () => {
    const result = resolveProfileSessionDefaults({
      workingDir: "/workspace",
      provider: "openai",
      model: "gpt-5-codex",
      profiles: {
        active_profile_id: "blank",
        profiles: [
          {
            id: "blank",
            name: "Blank",
            default_provider: "  ",
            default_model: "",
            default_workspace: "   ",
            api_key_overrides: null,
            created_at_ms: 1,
            updated_at_ms: 1,
          },
        ],
      },
    });

    assert.deepEqual(result, {
      workingDir: "/workspace",
      provider: "openai",
      model: "gpt-5-codex",
      profileId: "blank",
    });
  });
});
