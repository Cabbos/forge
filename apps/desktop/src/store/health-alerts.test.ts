import { describe, it } from "node:test";
import assert from "node:assert";
import { upsertHealthAlert } from "./health-alerts.ts";
import type { RuntimeHealthAlert } from "./types.ts";

describe("upsertHealthAlert", () => {
  const base: RuntimeHealthAlert = {
    alert_id: "alert-1",
    session_id: "session-1",
    level: "warn",
    title: "Session stale",
    message: "No events for 5 minutes.",
    remediation: "Check the session.",
  };

  it("adds the first alert", () => {
    const alerts = upsertHealthAlert([], base);
    assert.strictEqual(alerts.length, 1);
    assert.strictEqual(alerts[0].alert_id, "alert-1");
    assert.strictEqual(alerts[0].level, "warn");
  });

  it("replaces by alert_id instead of appending duplicate", () => {
    const alerts = upsertHealthAlert(
      upsertHealthAlert([], base),
      { ...base, message: "Updated stale message.", level: "critical" },
    );
    assert.strictEqual(alerts.length, 1);
    assert.strictEqual(alerts[0].message, "Updated stale message.");
    assert.strictEqual(alerts[0].level, "critical");
  });

  it("keeps multiple distinct alerts", () => {
    const alerts = upsertHealthAlert(
      upsertHealthAlert([], base),
      {
        alert_id: "alert-2",
        session_id: "session-2",
        level: "info",
        title: "Everything OK",
        message: "All systems nominal.",
      },
    );
    assert.strictEqual(alerts.length, 2);
    assert.strictEqual(alerts[0].alert_id, "alert-1");
    assert.strictEqual(alerts[1].alert_id, "alert-2");
  });

  it("replaces in the middle of a list", () => {
    const alerts = upsertHealthAlert(
      [
        base,
        {
          alert_id: "alert-2",
          session_id: "session-2",
          level: "info" as const,
          title: "T2",
          message: "M2",
        },
      ],
      { ...base, message: "Replaced first.", level: "critical" as const },
    );
    assert.strictEqual(alerts.length, 2);
    assert.strictEqual(alerts[0].message, "Replaced first.");
    assert.strictEqual(alerts[1].alert_id, "alert-2");
  });
});
