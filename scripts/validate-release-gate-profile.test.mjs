import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  REQUIRED_R3_LABELS,
  REQUIRED_R4_LABELS,
  validateReleaseHandoff,
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

const PRODUCER_COMMIT = "0a3d758d64c50b485b357c16d8eb221ffe193a31";
const RESULT_SHA = "a".repeat(64);

function passedGate(label, artifactId) {
  const gate = profile.gates.find((candidate) => candidate.label === label);
  return {
    id: gate.id,
    label: gate.label,
    command: gate.command,
    execution_status: "completed",
    condition_status: "passed",
    status: "passed",
    exit_code: 0,
    duration_ms: 1,
    started_at: "2026-07-15T00:00:00.000Z",
    finished_at: "2026-07-15T00:00:01.000Z",
    result_artifact_id: artifactId,
  };
}

function artifact(id, gateLabels, sha256 = RESULT_SHA) {
  return {
    id,
    schema_version: 2,
    generated_at: "2026-07-15T00:00:01.000Z",
    sha256,
    status: "passed",
    selected_gate_count: gateLabels.length,
    executed_gate_count: gateLabels.length,
    failed_gate_count: 0,
    failed_execution_count: 0,
    failed_condition_count: 0,
    unknown_condition_count: 0,
    gate_labels: gateLabels,
  };
}

function ownedEvidence(id, gateLabel, artifactId) {
  return {
    id,
    gate_label: gateLabel,
    execution_status: "completed",
    condition_status: "passed",
    result_artifact_id: artifactId,
  };
}

function validHandoff(state) {
  const requiredLabels = profile.gates
    .filter((gate) => gate.required_for.includes(state))
    .map((gate) => gate.label);
  const artifactId = `${state.toLowerCase()}-gate-results`;
  const handoff = {
    schema_version: 1,
    release_state: state,
    profile_id: profile.id,
    producer_commit: PRODUCER_COMMIT,
    result_artifacts: [artifact(artifactId, requiredLabels)],
    gates: requiredLabels.map((label) => passedGate(label, artifactId)),
    owned_evidence: [],
  };
  if (state === "R1") {
    const deterministic = "desktop deterministic signal cleanup";
    handoff.owned_evidence = [
      ownedEvidence("unified-memory-id-assertion", deterministic, artifactId),
      ownedEvidence("tailwind-4-warning-free-build", deterministic, artifactId),
      ownedEvidence("continuity-console-clean-fixture", deterministic, artifactId),
    ];
  }
  if (state === "R2") {
    const qualityLabel = "eval quality suite";
    const qualityArtifactId = "r2-eval-quality-results";
    handoff.result_artifacts.push(artifact(qualityArtifactId, [qualityLabel], "b".repeat(64)));
    handoff.gates.push(passedGate(qualityLabel, qualityArtifactId));
    handoff.owned_evidence = [
      ownedEvidence("strict-provider-identity", "eval execution identity baseline", artifactId),
      ownedEvidence("independent-workspace-observation", "eval independent workspace evidence baseline", artifactId),
      ownedEvidence("trusted-orchestration", "eval trusted execution baseline", artifactId),
      ownedEvidence("authenticated-fenced-worker", "eval authenticated fenced worker baseline", artifactId),
      ownedEvidence("full-eval-quality-suite", qualityLabel, qualityArtifactId),
    ];
  }
  return handoff;
}

test("release profile accepts complete commit-bound R1 and R2 handoffs", () => {
  for (const state of ["R1", "R2"]) {
    assert.deepEqual(
      validateReleaseHandoff(profile, validHandoff(state), {
        requiredState: state,
        expectedCommit: PRODUCER_COMMIT,
      }),
      { ok: true, errors: [] },
    );
  }
});

test("release profile rejects an R1 handoff with a missing label or failed condition", () => {
  const handoff = validHandoff("R1");
  handoff.gates.shift();
  handoff.gates[0].condition_status = "failed";
  const result = validateReleaseHandoff(profile, handoff, {
    requiredState: "R1",
    expectedCommit: PRODUCER_COMMIT,
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /missing required R1 handoff gate/);
  assert.match(result.errors.join("\n"), /failed condition/);
});

test("release profile rejects unknown provider, missing workspace, and stale-worker R2 evidence", () => {
  const handoff = validHandoff("R2");
  handoff.gates.find((gate) => gate.id === "eval-identity").condition_status = "unknown";
  handoff.owned_evidence = handoff.owned_evidence.filter(
    (evidence) => evidence.id !== "independent-workspace-observation",
  );
  handoff.gates.find((gate) => gate.id === "eval-worker").condition_status = "failed";
  const result = validateReleaseHandoff(profile, handoff, {
    requiredState: "R2",
    expectedCommit: PRODUCER_COMMIT,
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /eval execution identity baseline.*unknown/);
  assert.match(result.errors.join("\n"), /missing required R2 owned evidence: independent-workspace-observation/);
  assert.match(result.errors.join("\n"), /eval authenticated fenced worker baseline.*failed/);
});

test("release profile rejects handoffs from a different commit or malformed result artifact", () => {
  const handoff = validHandoff("R1");
  handoff.producer_commit = "f".repeat(40);
  handoff.result_artifacts[0].sha256 = "not-a-digest";
  const result = validateReleaseHandoff(profile, handoff, {
    requiredState: "R1",
    expectedCommit: PRODUCER_COMMIT,
  });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /producer_commit does not match/);
  assert.match(result.errors.join("\n"), /sha256/);
});
