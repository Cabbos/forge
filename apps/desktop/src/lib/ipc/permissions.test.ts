import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  listPermissionRules,
  resetPermissionRule,
  setPermissionRule,
} from "./permissions.ts";

describe("permission IPC fallbacks", () => {
  it("returns no rules outside the Tauri runtime", async () => {
    assert.deepEqual(await listPermissionRules(), []);
  });

  it("throws clear errors for permission mutations outside Tauri", async () => {
    await assert.rejects(
      setPermissionRule({ toolName: "write_to_file", decision: "deny" }),
      /Permission rule mutation is not available outside Tauri runtime/,
    );
    await assert.rejects(
      resetPermissionRule("write_to_file"),
      /Permission rule reset is not available outside Tauri runtime/,
    );
  });
});
