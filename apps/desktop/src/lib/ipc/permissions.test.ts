import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  getPermissionMode,
  listPermissionRules,
  resetPermissionRule,
  setPermissionMode,
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

  it("returns manual permission mode outside the Tauri runtime", async () => {
    assert.deepEqual(await getPermissionMode("session-1"), {
      mode: "manual_confirm",
      workspace_path: null,
      session_scoped: true,
    });
  });

  it("throws clear errors for permission mode mutations outside Tauri", async () => {
    await assert.rejects(
      setPermissionMode({
        sessionId: "session-1",
        mode: "full_access",
        workspacePath: "/tmp/demo",
      }),
      /Permission mode mutation is not available outside Tauri runtime/,
    );
  });
});
