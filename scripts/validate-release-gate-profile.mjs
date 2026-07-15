import { existsSync, readFileSync } from "node:fs";

export const REQUIRED_R3_LABELS = Object.freeze([
  "desktop deterministic signal cleanup",
  "desktop command execution safety baseline",
  "desktop credential and redaction safety baseline",
  "desktop checkpoint restore safety baseline",
  "desktop CSP and capability safety baseline",
  "eval execution identity baseline",
  "eval independent workspace evidence baseline",
  "eval trusted execution baseline",
  "eval authenticated fenced worker baseline",
  "acceptance matrix contract tests",
  "desktop frontend architecture",
  "desktop protocol sync",
  "website production build",
  "eval quality suite",
  "release manifest contract validation",
]);

export const REQUIRED_R4_LABELS = Object.freeze([
  "macOS signing configuration contract",
  "public beta artifact verification contract",
  "public beta install evidence contract",
  "website verified download contract",
]);

const SHA256 = /^[0-9a-f]{64}$/i;
const SHA40 = /^[0-9a-f]{40}$/i;
const HANDOFF_STATES = new Set(["R1", "R2"]);
const REQUIRED_OWNED_EVIDENCE = Object.freeze({
  R1: Object.freeze([
    "unified-memory-id-assertion",
    "tailwind-4-warning-free-build",
    "continuity-console-clean-fixture",
  ]),
  R2: Object.freeze([
    "strict-provider-identity",
    "independent-workspace-observation",
    "trusted-orchestration",
    "authenticated-fenced-worker",
    "full-eval-quality-suite",
  ]),
});

export function validateReleaseGateProfile(profile, { requiredState = null } = {}) {
  const errors = [];
  if (!profile || typeof profile !== "object" || Array.isArray(profile)) return { ok: false, errors: ["profile must be an object"] };
  if (profile.version !== 1) errors.push("version must be 1");
  if (typeof profile.id !== "string" || !profile.id) errors.push("id is required");
  if (!Array.isArray(profile.gates)) errors.push("gates must be an array");
  const labels = new Set();
  for (const [index, gate] of (profile.gates ?? []).entries()) {
    if (!gate || typeof gate.label !== "string" || !gate.label) errors.push(`gates[${index}].label is required`);
    if (labels.has(gate?.label)) errors.push(`duplicate gate label: ${gate.label}`);
    if (gate?.label) labels.add(gate.label);
    if (!Array.isArray(gate?.required_for) || gate.required_for.length === 0) errors.push(`gates[${index}].required_for is required`);
  }
  if (requiredState === "R3") {
    for (const label of REQUIRED_R3_LABELS) {
      const gate = (profile.gates ?? []).find((candidate) => candidate.label === label);
      if (!gate) errors.push(`missing required R3 gate: ${label}`);
      else if (!gate.required_for.includes("R3")) errors.push(`gate is not required for R3: ${label}`);
    }
  }
  if (requiredState === "R4") {
    for (const label of [...REQUIRED_R3_LABELS, ...REQUIRED_R4_LABELS]) {
      const gate = (profile.gates ?? []).find((candidate) => candidate.label === label);
      if (!gate) errors.push(`missing required R4 gate: ${label}`);
      else if (!gate.required_for.includes("R4") && !gate.required_for.includes("R3")) {
        errors.push(`gate is not part of the R4 lineage: ${label}`);
      }
    }
  }
  return { ok: errors.length === 0, errors };
}

export function validateReleaseHandoff(
  profile,
  handoff,
  { requiredState = null, expectedCommit = null } = {},
) {
  const errors = [];
  const profileResult = validateReleaseGateProfile(profile);
  if (!profileResult.ok) return { ok: false, errors: profileResult.errors.map((error) => `profile: ${error}`) };
  if (!handoff || typeof handoff !== "object" || Array.isArray(handoff)) {
    return { ok: false, errors: ["handoff must be an object"] };
  }
  if (handoff.schema_version !== 1) errors.push("schema_version must be 1");
  if (!HANDOFF_STATES.has(handoff.release_state)) errors.push("release_state must be R1 or R2");
  if (requiredState && handoff.release_state !== requiredState) {
    errors.push(`release_state must be ${requiredState}`);
  }
  const state = requiredState ?? handoff.release_state;
  if (handoff.profile_id !== profile.id) errors.push(`profile_id must be ${profile.id}`);
  if (typeof handoff.producer_commit !== "string" || !SHA40.test(handoff.producer_commit)) {
    errors.push("producer_commit must be a full 40-character SHA");
  } else if (expectedCommit && handoff.producer_commit !== expectedCommit) {
    errors.push(`producer_commit does not match expected commit ${expectedCommit}`);
  }

  if (!Array.isArray(handoff.result_artifacts) || handoff.result_artifacts.length === 0) {
    errors.push("result_artifacts must be a non-empty array");
  }
  const artifacts = new Map();
  for (const [index, artifact] of (handoff.result_artifacts ?? []).entries()) {
    const prefix = `result_artifacts[${index}]`;
    if (!artifact || typeof artifact.id !== "string" || !artifact.id) {
      errors.push(`${prefix}.id is required`);
      continue;
    }
    if (artifacts.has(artifact.id)) errors.push(`duplicate result artifact id: ${artifact.id}`);
    artifacts.set(artifact.id, artifact);
    if (artifact.schema_version !== 2) errors.push(`${prefix}.schema_version must be 2`);
    if (typeof artifact.generated_at !== "string" || Number.isNaN(Date.parse(artifact.generated_at))) {
      errors.push(`${prefix}.generated_at must be an ISO timestamp`);
    }
    if (typeof artifact.sha256 !== "string" || !SHA256.test(artifact.sha256)) {
      errors.push(`${prefix}.sha256 must be a SHA-256 digest`);
    }
    if (artifact.status !== "passed") errors.push(`${prefix}.status must be passed`);
    if (!Array.isArray(artifact.gate_labels) || artifact.gate_labels.length === 0) {
      errors.push(`${prefix}.gate_labels must be a non-empty array`);
    }
    const uniqueLabels = new Set(artifact.gate_labels ?? []);
    if (uniqueLabels.size !== (artifact.gate_labels ?? []).length) {
      errors.push(`${prefix}.gate_labels contains duplicates`);
    }
    if (artifact.selected_gate_count !== (artifact.gate_labels ?? []).length) {
      errors.push(`${prefix}.selected_gate_count does not match gate_labels`);
    }
    if (artifact.executed_gate_count !== artifact.selected_gate_count) {
      errors.push(`${prefix}.executed_gate_count does not match selected_gate_count`);
    }
    for (const field of [
      "failed_gate_count",
      "failed_execution_count",
      "failed_condition_count",
      "unknown_condition_count",
    ]) {
      if (artifact[field] !== 0) errors.push(`${prefix}.${field} must be 0`);
    }
  }

  if (!Array.isArray(handoff.gates)) errors.push("gates must be an array");
  const gates = new Map();
  const gateArtifactLabels = new Map();
  for (const [index, gate] of (handoff.gates ?? []).entries()) {
    const prefix = `gates[${index}]`;
    if (!gate || typeof gate.label !== "string" || !gate.label) {
      errors.push(`${prefix}.label is required`);
      continue;
    }
    if (gates.has(gate.label)) errors.push(`duplicate handoff gate label: ${gate.label}`);
    gates.set(gate.label, gate);
    const profileGate = profile.gates.find((candidate) => candidate.label === gate.label);
    if (!profileGate) {
      errors.push(`unclassified handoff gate: ${gate.label}`);
      continue;
    }
    if (gate.id !== profileGate.id) errors.push(`${gate.label} id does not match release profile`);
    if (gate.command !== profileGate.command) errors.push(`${gate.label} command does not match release profile`);
    if (gate.execution_status !== "completed") {
      errors.push(`${gate.label} has ${gate.execution_status ?? "missing"} execution`);
    }
    if (gate.condition_status !== "passed") {
      errors.push(`${gate.label} has ${gate.condition_status ?? "missing"} condition`);
    }
    if (gate.status !== gate.condition_status) errors.push(`${gate.label} legacy status does not match condition_status`);
    if (gate.exit_code !== 0) errors.push(`${gate.label} exit_code must be 0`);
    if (!Number.isFinite(gate.duration_ms) || gate.duration_ms < 0) errors.push(`${gate.label} duration_ms is invalid`);
    if (typeof gate.started_at !== "string" || Number.isNaN(Date.parse(gate.started_at))) {
      errors.push(`${gate.label} started_at must be an ISO timestamp`);
    }
    if (typeof gate.finished_at !== "string" || Number.isNaN(Date.parse(gate.finished_at))) {
      errors.push(`${gate.label} finished_at must be an ISO timestamp`);
    }
    const artifact = artifacts.get(gate.result_artifact_id);
    if (!artifact) {
      errors.push(`${gate.label} references unknown result artifact ${gate.result_artifact_id ?? "<missing>"}`);
    } else {
      const labels = gateArtifactLabels.get(artifact.id) ?? [];
      labels.push(gate.label);
      gateArtifactLabels.set(artifact.id, labels);
      if (!artifact.gate_labels.includes(gate.label)) {
        errors.push(`${gate.label} is absent from result artifact ${artifact.id}`);
      }
    }
  }

  if (HANDOFF_STATES.has(state)) {
    const requiredLabels = profile.gates
      .filter((gate) => gate.required_for.includes(state))
      .map((gate) => gate.label);
    for (const label of requiredLabels) {
      if (!gates.has(label)) errors.push(`missing required ${state} handoff gate: ${label}`);
    }
  }

  for (const [artifactId, artifact] of artifacts) {
    const recorded = [...(artifact.gate_labels ?? [])].sort();
    const referenced = [...(gateArtifactLabels.get(artifactId) ?? [])].sort();
    if (JSON.stringify(recorded) !== JSON.stringify(referenced)) {
      errors.push(`result artifact ${artifactId} selected-vs-executed gate labels do not match`);
    }
  }

  if (!Array.isArray(handoff.owned_evidence)) errors.push("owned_evidence must be an array");
  const evidenceById = new Map();
  for (const [index, evidence] of (handoff.owned_evidence ?? []).entries()) {
    const prefix = `owned_evidence[${index}]`;
    if (!evidence || typeof evidence.id !== "string" || !evidence.id) {
      errors.push(`${prefix}.id is required`);
      continue;
    }
    if (evidenceById.has(evidence.id)) errors.push(`duplicate owned evidence id: ${evidence.id}`);
    evidenceById.set(evidence.id, evidence);
    if (!gates.has(evidence.gate_label)) errors.push(`${evidence.id} references unknown gate ${evidence.gate_label ?? "<missing>"}`);
    if (!artifacts.has(evidence.result_artifact_id)) {
      errors.push(`${evidence.id} references unknown result artifact ${evidence.result_artifact_id ?? "<missing>"}`);
    }
    if (evidence.execution_status !== "completed") errors.push(`${evidence.id} execution is not completed`);
    if (evidence.condition_status !== "passed") errors.push(`${evidence.id} condition is not passed`);
  }
  for (const evidenceId of REQUIRED_OWNED_EVIDENCE[state] ?? []) {
    if (!evidenceById.has(evidenceId)) errors.push(`missing required ${state} owned evidence: ${evidenceId}`);
  }

  return { ok: errors.length === 0, errors };
}

function main(argv) {
  const index = argv.indexOf("--release-profile");
  const path = index >= 0 ? argv[index + 1] : null;
  const stateIndex = argv.indexOf("--require-state");
  const requiredState = stateIndex >= 0 ? argv[stateIndex + 1] : null;
  if (!path || !existsSync(path)) {
    console.error("--release-profile must point to an existing JSON file");
    return 2;
  }
  let profile;
  try {
    profile = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 2;
  }
  const result = validateReleaseGateProfile(profile, { requiredState });
  console.log(JSON.stringify(result, null, 2));
  return result.ok ? 0 : 1;
}

if (process.argv[1] === new URL(import.meta.url).pathname) process.exitCode = main(process.argv.slice(2));
