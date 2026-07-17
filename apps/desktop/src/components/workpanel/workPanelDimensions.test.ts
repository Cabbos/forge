import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  DEFAULT_WORK_PANEL_WIDTH_PERCENT,
  getWorkPanelBounds,
  getWorkPanelViewportMode,
  normalizeWorkPanelWidthPercent,
} from "./workPanelDimensions.ts";

describe("work panel dimensions", () => {
  it("normalizes persisted widths to the supported range", () => {
    assert.equal(DEFAULT_WORK_PANEL_WIDTH_PERCENT, 40);
    assert.equal(normalizeWorkPanelWidthPercent(undefined), 40);
    assert.equal(normalizeWorkPanelWidthPercent(12), 34);
    assert.equal(normalizeWorkPanelWidthPercent(90), 62);
  });

  it("selects the responsive mode at the agreed breakpoints", () => {
    assert.equal(getWorkPanelViewportMode(1100), "split");
    assert.equal(getWorkPanelViewportMode(899), "fixed");
    assert.equal(getWorkPanelViewportMode(719), "overlay");
  });

  it("derives bounded split widths from the available workbench", () => {
    assert.deepEqual(getWorkPanelBounds(1000), { min: 36, max: 62 });
    assert.deepEqual(getWorkPanelBounds(2000), { min: 34, max: 46 });
    assert.deepEqual(getWorkPanelBounds(800), { min: 45, max: 62 });
    assert.deepEqual(getWorkPanelBounds(2400), { min: 34, max: 38.33 });
    assert.deepEqual(getWorkPanelBounds(0), { min: 62, max: 62 });
  });
});
