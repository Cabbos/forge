import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { validateReleaseGateProfile, REQUIRED_R3_LABELS } from "./validate-release-gate-profile.mjs";

const profile = JSON.parse(readFileSync(new URL("../release/release-gates.v1.json", import.meta.url), "utf8"));

test("public beta profile contains every fixed R3 gate", () => {
  const result = validateReleaseGateProfile(profile, { requiredState: "R3" });
  assert.deepEqual(result, { ok: true, errors: [] });
  for (const label of REQUIRED_R3_LABELS) {
    assert.ok(profile.gates.some((gate) => gate.label === label), label);
  }
});

test("profile rejects missing or duplicate required labels", () => {
  const broken = structuredClone(profile);
  broken.gates = broken.gates.filter((gate) => gate.label !== REQUIRED_R3_LABELS[0]);
  broken.gates.push({ ...broken.gates[0], id: "duplicate", label: broken.gates[1].label });
  const result = validateReleaseGateProfile(broken, { requiredState: "R3" });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /missing required R3 gate/);
  assert.match(result.errors.join("\n"), /duplicate gate label/);
});
