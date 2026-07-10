import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  REQUIRED_R3_LABELS,
  REQUIRED_R4_LABELS,
  validateReleaseGateProfile,
} from "./validate-release-gate-profile.mjs";

const profile = JSON.parse(readFileSync(new URL("../release/release-gates.v1.json", import.meta.url), "utf8"));

test("public beta profile contains every fixed R3 gate", () => {
  const result = validateReleaseGateProfile(profile, { requiredState: "R3" });
  assert.deepEqual(result, { ok: true, errors: [] });
  for (const label of REQUIRED_R3_LABELS) {
    assert.ok(profile.gates.some((gate) => gate.label === label), label);
  }
});

test("every fixed R3 gate is reachable through the acceptance matrix with the same command", () => {
  const acceptance = JSON.parse(
    execFileSync(fileURLToPath(new URL("acceptance.sh", import.meta.url)), ["--list-json"], {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    }),
  );
  for (const label of REQUIRED_R3_LABELS) {
    const profileGate = profile.gates.find((gate) => gate.label === label);
    const acceptanceGate = acceptance.gates.find((gate) => gate.label === label);
    assert.ok(acceptanceGate, `acceptance gate is missing: ${label}`);
    assert.equal(acceptanceGate.command, profileGate.command, `command drift for ${label}`);
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

test("public beta profile locks the four fixed R4 distribution labels", () => {
  assert.deepEqual(validateReleaseGateProfile(profile, { requiredState: "R4" }), {
    ok: true,
    errors: [],
  });
  assert.deepEqual(
    profile.gates.filter((gate) => gate.required_for.includes("R4")).map((gate) => gate.label),
    [...REQUIRED_R4_LABELS],
  );
});
