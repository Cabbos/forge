import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  resolveProfileComposerDefaults,
  resolveProfileSessionDefaults,
} from "./sessionProfileDefaults.ts";
import type { ForgeProfile, ProfileListPayload } from "../lib/ipc/types.ts";

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

function forgeProfile(overrides: Partial<ForgeProfile>): ForgeProfile {
  return {
    id: overrides.id ?? "profile-1",
    name: overrides.name ?? "Profile",
    default_provider: overrides.default_provider ?? null,
    default_model: overrides.default_model ?? null,
    default_workspace: overrides.default_workspace ?? null,
    api_key_overrides: overrides.api_key_overrides ?? null,
    created_at_ms: overrides.created_at_ms ?? 1,
    updated_at_ms: overrides.updated_at_ms ?? 1,
  };
}

describe("resolveProfileComposerDefaults", () => {
  it("uses profile provider and model for the visible composer selection", () => {
    const result = resolveProfileComposerDefaults({
      currentProvider: "deepseek",
      currentModel: "deepseek-chat",
      profile: forgeProfile({
        default_provider: "anthropic",
        default_model: "claude-sonnet-4-6",
      }),
    });

    assert.deepEqual(result, {
      provider: "anthropic",
      model: "claude-sonnet-4-6",
      changed: true,
    });
  });

  it("uses the provider default model when profile only sets provider", () => {
    const result = resolveProfileComposerDefaults({
      currentProvider: "deepseek",
      currentModel: "deepseek-v4-flash[1m]",
      profile: forgeProfile({ default_provider: "openai" }),
    });

    assert.deepEqual(result, {
      provider: "openai",
      model: "gpt-4o",
      changed: true,
    });
  });

  it("infers provider from a known profile model when provider is omitted", () => {
    const result = resolveProfileComposerDefaults({
      currentProvider: "deepseek",
      currentModel: "deepseek-v4-flash[1m]",
      profile: forgeProfile({ default_model: "claude-opus-4-7" }),
    });

    assert.deepEqual(result, {
      provider: "anthropic",
      model: "claude-opus-4-7",
      changed: true,
    });
  });

  it("does not change composer selection when profile has no model defaults", () => {
    const result = resolveProfileComposerDefaults({
      currentProvider: "openai",
      currentModel: "gpt-4o",
      profile: forgeProfile({}),
    });

    assert.deepEqual(result, {
      provider: "openai",
      model: "gpt-4o",
      changed: false,
    });
  });
});
