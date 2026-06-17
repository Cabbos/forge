import assert from "node:assert/strict";
import test from "node:test";

import { summarizeReview } from "./a2aReview.ts";

test("summarizes open and resolved review findings", () => {
  const summary = summarizeReview([
    { severity: "error", resolved: false },
    { severity: "warning", resolved: false },
    { severity: "warning", resolved: true },
    { severity: "info", resolved: true }
  ]);

  assert.deepEqual(summary, {
    total: 4,
    openErrors: 1,
    openWarnings: 1,
    resolved: 2
  });
});
