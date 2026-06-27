import { describe, it } from "node:test";
import assert from "node:assert";
import {
  clearStaleSessionHealthAlerts,
  upsertHealthAlert,
  visibleHealthAlertsForSession,
} from "./health-alerts.ts";
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

describe("clearStaleSessionHealthAlerts", () => {
  const staleSessionOne: RuntimeHealthAlert = {
    alert_id: "session-stale-session-1",
    session_id: "session-1",
    level: "warn",
    title: "会话无响应",
    message: "No events for session 1.",
  };
  const staleSessionTwo: RuntimeHealthAlert = {
    alert_id: "session-stale-session-2",
    session_id: "session-2",
    level: "warn",
    title: "会话无响应",
    message: "No events for session 2.",
  };
  const missingApiKey: RuntimeHealthAlert = {
    alert_id: "missing-api-key:session-1",
    session_id: "session-1",
    level: "critical",
    title: "缺少模型密钥",
    message: "Missing key.",
  };

  it("clears only the stale alert for the fresh session", () => {
    const alerts = clearStaleSessionHealthAlerts(
      [staleSessionOne, staleSessionTwo, missingApiKey],
      "session-1",
    );

    assert.deepStrictEqual(
      alerts.map((alert) => alert.alert_id),
      ["session-stale-session-2", "missing-api-key:session-1"],
    );
  });

  it("keeps alerts unchanged when the session has no stale alert", () => {
    const alerts = [staleSessionTwo, missingApiKey];

    assert.deepStrictEqual(clearStaleSessionHealthAlerts(alerts, "session-1"), alerts);
  });
});

describe("visibleHealthAlertsForSession", () => {
  const staleSessionOne: RuntimeHealthAlert = {
    alert_id: "session-stale-session-1",
    session_id: "session-1",
    level: "warn",
    title: "会话无响应",
    message: "No events for session 1.",
  };
  const staleSessionTwo: RuntimeHealthAlert = {
    alert_id: "session-stale-session-2",
    session_id: "session-2",
    level: "warn",
    title: "会话无响应",
    message: "No events for session 2.",
  };
  const missingApiKey: RuntimeHealthAlert = {
    alert_id: "missing-api-key:session-1",
    session_id: "session-1",
    level: "critical",
    title: "缺少模型密钥",
    message: "Missing key.",
  };

  it("keeps only current-session stale alerts while preserving global alerts", () => {
    const visible = visibleHealthAlertsForSession(
      [staleSessionOne, staleSessionTwo, missingApiKey],
      "session-1",
    );

    assert.deepStrictEqual(
      visible.map((alert) => alert.alert_id),
      ["session-stale-session-1", "missing-api-key:session-1"],
    );
  });

  it("shows all alerts before an active session is selected", () => {
    const alerts = [staleSessionOne, missingApiKey];

    assert.deepStrictEqual(visibleHealthAlertsForSession(alerts, null), alerts);
  });
});
