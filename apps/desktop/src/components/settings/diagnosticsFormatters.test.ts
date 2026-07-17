import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  formatDetailValue,
  formatDiagnosticDetail,
  formatDuration,
  formatGatewayDegradedMode,
  formatRuntimeTaskName,
} from "./diagnosticsFormatters.ts";

describe("formatRuntimeTaskName", () => {
  it("maps known runtime tasks to short labels", () => {
    assert.equal(formatRuntimeTaskName("webhook_listener"), "webhook");
    assert.equal(formatRuntimeTaskName("trigger_runner"), "trigger");
    assert.equal(formatRuntimeTaskName("scheduler_tick"), "scheduler");
  });

  it("humanizes unknown task names", () => {
    assert.equal(formatRuntimeTaskName("memory_recall_worker"), "memory recall worker");
    assert.equal(formatRuntimeTaskName("watchdog"), "watchdog");
  });
});

describe("formatGatewayDegradedMode", () => {
  it("returns null when degraded mode is inactive or absent", () => {
    assert.equal(formatGatewayDegradedMode(undefined), null);
    assert.equal(
      formatGatewayDegradedMode({
        active: false,
        reason: "",
        fallback: "",
        input_policy: "",
        confirmation_policy: "",
      }),
      null,
    );
  });

  it("renders reason, fallback, and recovery with defaults", () => {
    assert.equal(
      formatGatewayDegradedMode({
        active: true,
        reason: "",
        fallback: "",
        input_policy: "",
        confirmation_policy: "",
      }),
      "Gateway degraded mode is active. · fallback desktop_runtime · recovery forge service restart",
    );
    assert.equal(
      formatGatewayDegradedMode({
        active: true,
        reason: "Gateway unavailable.",
        fallback: "local",
        input_policy: "",
        confirmation_policy: "",
        recovery_command: "forge service restart --force",
      }),
      "Gateway unavailable. · fallback local · recovery forge service restart --force",
    );
  });
});

describe("formatDiagnosticDetail", () => {
  it("handles null, primitives, and empty objects", () => {
    assert.equal(formatDiagnosticDetail(null), "");
    assert.equal(formatDiagnosticDetail(undefined), "");
    assert.equal(formatDiagnosticDetail("plain"), "plain");
    assert.equal(formatDiagnosticDetail(42), "42");
    assert.equal(formatDiagnosticDetail({}), "{}");
  });

  it("joins entries with separators and truncates to four keys", () => {
    const detail = { a: 1, b: 2, c: 3, d: 4, e: 5 };
    assert.equal(formatDiagnosticDetail(detail), "a: 1 · b: 2 · c: 3 · d: 4");
  });
});

describe("formatDetailValue", () => {
  it("summarizes arrays and objects, passes primitives through", () => {
    assert.equal(formatDetailValue([1, 2, 3]), "[3]");
    assert.equal(formatDetailValue({ nested: true }), "{...}");
    assert.equal(formatDetailValue("value"), "value");
    assert.equal(formatDetailValue(null), "null");
  });
});

describe("formatDuration", () => {
  it("formats seconds, minutes, and hours", () => {
    assert.equal(formatDuration(45), "45s");
    assert.equal(formatDuration(90), "1m");
    assert.equal(formatDuration(3599), "59m");
    assert.equal(formatDuration(3600), "1h");
    assert.equal(formatDuration(7320), "2h");
  });
});
