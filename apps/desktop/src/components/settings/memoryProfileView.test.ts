import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  buildMemoryFactUpsertInput,
  resolveActiveMemoryProfile,
  resolveMemoryProfileId,
} from "./memoryProfileView.ts";
import type { MemoryFact, ProfileListPayload } from "../../lib/ipc/types.ts";

const profiles: ProfileListPayload = {
  active_profile_id: "work",
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
      id: "work",
      name: "Work",
      default_provider: "anthropic",
      default_model: "claude-sonnet-4-5",
      default_workspace: "/Users/cabbos/project/work",
      api_key_overrides: null,
      created_at_ms: 2,
      updated_at_ms: 2,
    },
  ],
};

function fact(overrides: Partial<MemoryFact>): MemoryFact {
  return {
    id: overrides.id ?? "fact-1",
    text: overrides.text ?? "Existing fact",
    tags: overrides.tags ?? [],
    profile_id: overrides.profile_id ?? null,
    source: overrides.source ?? null,
    created_at_ms: overrides.created_at_ms ?? 1,
    updated_at_ms: overrides.updated_at_ms ?? 1,
  };
}

describe("resolveActiveMemoryProfile", () => {
  it("returns the active profile when present", () => {
    assert.deepEqual(resolveActiveMemoryProfile(profiles), profiles.profiles[1]);
  });

  it("returns null when the active profile id is missing", () => {
    assert.equal(
      resolveActiveMemoryProfile({ ...profiles, active_profile_id: "missing" }),
      null,
    );
  });
});

describe("resolveMemoryProfileId", () => {
  it("returns a trimmed active profile id", () => {
    assert.equal(resolveMemoryProfileId("  work  "), "work");
  });

  it("returns null for blank profile ids", () => {
    assert.equal(resolveMemoryProfileId("   "), null);
    assert.equal(resolveMemoryProfileId(null), null);
  });
});

describe("buildMemoryFactUpsertInput", () => {
  it("attaches the active profile id when creating a fact", () => {
    assert.deepEqual(
      buildMemoryFactUpsertInput({
        text: "Use launchd for gateway autostart",
        tags: ["runtime", "gateway"],
        activeProfileId: "work",
      }),
      {
        text: "Use launchd for gateway autostart",
        tags: ["runtime", "gateway"],
        profile_id: "work",
      },
    );
  });

  it("preserves an existing fact profile id when updating", () => {
    assert.deepEqual(
      buildMemoryFactUpsertInput({
        fact: fact({ id: "fact-1", profile_id: "personal" }),
        text: "Updated",
        tags: [],
        activeProfileId: "work",
      }),
      {
        id: "fact-1",
        text: "Updated",
        tags: [],
        profile_id: "personal",
      },
    );
  });

  it("omits blank profile ids from the upsert payload", () => {
    assert.deepEqual(
      buildMemoryFactUpsertInput({
        text: "Global",
        tags: [],
        activeProfileId: "   ",
      }),
      {
        text: "Global",
        tags: [],
      },
    );
  });
});
