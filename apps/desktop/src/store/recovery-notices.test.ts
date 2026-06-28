import { describe, it } from "node:test";
import assert from "node:assert";
import type { StreamEvent } from "../lib/protocol.ts";
import { upsertRecoveryNotice } from "./recovery-notices.ts";

describe("upsertRecoveryNotice", () => {
  it("records recovery notices without a session store", () => {
    const event: Extract<StreamEvent, { event_type: "recovery_notice" }> = {
      event_type: "recovery_notice",
      session_id: "missing-session",
      notice_id: "notice-1",
      title: "Session restore failed",
      message: "Forge started fresh.",
      reason: "snapshot_restore_failed",
      recoverable: false,
    };

    const notices = upsertRecoveryNotice([], event);

    assert.strictEqual(notices.length, 1);
    assert.strictEqual(notices[0].notice_id, "notice-1");
    assert.strictEqual(notices[0].session_id, "missing-session");
  });

  it("replaces duplicate notice ids instead of appending", () => {
    const base: Extract<StreamEvent, { event_type: "recovery_notice" }> = {
      event_type: "recovery_notice",
      session_id: "session-1",
      notice_id: "notice-1",
      title: "Session restore failed",
      message: "First message.",
      reason: "snapshot_restore_failed",
      recoverable: false,
    };

    const notices = upsertRecoveryNotice(
      upsertRecoveryNotice([], base),
      { ...base, message: "Updated message.", recoverable: true },
    );

    assert.strictEqual(notices.length, 1);
    assert.strictEqual(notices[0].message, "Updated message.");
    assert.strictEqual(notices[0].recoverable, true);
  });
});
