import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { buildDiagnosticRepairAction } from "./diagnosticsRepairView.ts";

describe("buildDiagnosticRepairAction", () => {
  it("returns a labeled action for warning checks with a repair action id", () => {
    const action = buildDiagnosticRepairAction({
      id: "gateway_service",
      label: "Gateway service",
      status: "warn",
      message: "Gateway service is installed but not running.",
      repairActionId: "restart_gateway",
    });

    assert.deepEqual(action, {
      actionId: "restart_gateway",
      label: "重启 Gateway",
    });
  });

  it("hides actions for passing checks", () => {
    const action = buildDiagnosticRepairAction({
      id: "gateway_service",
      label: "Gateway service",
      status: "pass",
      message: "Gateway service is installed and running.",
      repairActionId: "restart_gateway",
    });

    assert.equal(action, null);
  });
});
