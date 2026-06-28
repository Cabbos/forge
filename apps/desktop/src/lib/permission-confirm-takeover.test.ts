import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { BlockState } from "./protocol.ts";
import { findLatestPendingWorkspaceConfirm } from "./permission-confirm-takeover.ts";

const workspace = "/Users/cabbos/project/forge";

describe("permission confirmation takeover", () => {
  it("full access selects the latest pending same-workspace confirmation", () => {
    const blocks = [
      confirmBlock("older-write", {
        operation: "write_file",
        affected_files: ["src/old.ts"],
      }),
      confirmBlock("latest-shell", {
        operation: "shell",
        command: "npm test",
        affected_files: [],
      }),
    ];

    const selected = findLatestPendingWorkspaceConfirm(blocks, workspace, { allowAnyOperation: true });

    assert.equal(selected?.block_id, "latest-shell");
  });

  it("full access does not select external, home-relative, or traversal targets", () => {
    const blockedTargets = [
      "/Users/cabbos/.ssh/config",
      "~/.forge/config.json",
      "../outside.ts",
      "src/../.env",
    ];

    for (const target of blockedTargets) {
      const selected = findLatestPendingWorkspaceConfirm(
        [confirmBlock(`blocked-${target}`, { affected_files: [target] })],
        workspace,
        { allowAnyOperation: true },
      );

      assert.equal(selected, null, `${target} should stay manual`);
    }
  });

  it("project trust selects normal writes but not shell or sensitive dotenv files", () => {
    const blocks = [
      confirmBlock("trusted-write", {
        operation: "edit_file",
        affected_files: ["src/App.tsx"],
      }),
      confirmBlock("trusted-shell", {
        operation: "shell",
        command: "npm install left-pad",
        affected_files: [],
      }),
      confirmBlock("trusted-env", {
        operation: "write_file",
        affected_files: [".env.local"],
      }),
    ];

    const selected = findLatestPendingWorkspaceConfirm(blocks, workspace, { allowAnyOperation: false });

    assert.equal(selected?.block_id, "trusted-write");
  });

  it("skips resolved, interrupted, and different-workspace confirmations", () => {
    const blocks = [
      confirmBlock("different-workspace", {
        workspace_path: "/Users/cabbos/project/other",
        operation: "write_file",
        affected_files: ["src/App.tsx"],
      }),
      confirmBlock("interrupted", { affected_files: ["src/interrupted.ts"] }, { confirm_interrupted: true }),
      confirmBlock("confirmed", { affected_files: ["src/confirmed.ts"] }, { confirmed: true }),
    ];

    const selected = findLatestPendingWorkspaceConfirm(blocks, workspace, { allowAnyOperation: true });

    assert.equal(selected, null);
  });
});

function confirmBlock(
  blockId: string,
  boundary: Record<string, unknown>,
  metadata: Record<string, unknown> = {},
): BlockState {
  return {
    block_id: blockId,
    event_type: "confirm_ask",
    content: "Confirm",
    isComplete: true,
    metadata: {
      ...metadata,
      boundary: {
        workspace_name: "forge",
        workspace_path: workspace,
        operation: "write_file",
        risk_level: "medium",
        checkpoint_status: "ready",
        ...boundary,
      },
    },
  };
}
