import { existsSync, readFileSync } from "node:fs";

const SHA256 = /^[0-9a-f]{64}$/i;
const SHA40 = /^[0-9a-f]{40}$/i;
const RELEASE_STATES = new Set(["R0", "R1", "R2", "R3", "R4"]);
const EXECUTION_STATES = new Set(["not_started", "running", "completed", "execution_failed"]);
const CONDITION_STATES = new Set(["passed", "failed", "manual", "unknown"]);
const REQUIRED_INPUT_ROLES = Object.freeze([
  "acceptance_results",
  "desktop_safety",
  "eval_trustworthiness",
  "gitnexus",
  "representative_mock",
  "representative_real_forge",
]);
const REQUIRED_FIELDS = Object.freeze([
  "schema_version",
  "release_state",
  "version",
  "commit_sha",
  "source_branch",
  "generated_at",
  "profile_id",
  "profile_sha256",
  "lockfiles",
  "selected_gates",
  "gate_summary",
  "gates",
  "input_bindings",
  "eval_evidence",
  "artifacts",
  "signing",
  "notarization",
  "installation_smoke",
  "website",
  "previous_release",
  "gitnexus",
  "residual_risks",
]);

function validTimestamp(value) {
  return typeof value === "string" && !Number.isNaN(Date.parse(value));
}

function duplicates(values) {
  const seen = new Set();
  return values.filter((value) => {
    if (seen.has(value)) return true;
    seen.add(value);
    return false;
  });
}

function sameMembers(left, right) {
  if (left.length !== right.length) return false;
  const sortedLeft = [...left].sort();
  const sortedRight = [...right].sort();
  return sortedLeft.every((value, index) => value === sortedRight[index]);
}

export function validateReleaseManifest(manifest, { requiredState = null, profile = null } = {}) {
  const structuralErrors = [];
  const stateErrors = [];
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    structuralErrors.push("manifest must be an object");
    return { ok: false, errors: structuralErrors, structuralErrors, stateErrors };
  }
  for (const field of REQUIRED_FIELDS) {
    if (!Object.hasOwn(manifest, field)) structuralErrors.push(`${field} is required`);
  }
  if (manifest.schema_version !== 1) structuralErrors.push("schema_version must be 1");
  if (!RELEASE_STATES.has(manifest.release_state)) {
    structuralErrors.push("release_state must be R0, R1, R2, R3, or R4");
  }
  if (requiredState && manifest.release_state !== requiredState) {
    stateErrors.push(`release_state must be ${requiredState}`);
  }
  if (typeof manifest.version !== "string" || !/^desktop-v\d+\.\d+\.\d+-beta\.\d+$/.test(manifest.version)) {
    structuralErrors.push("version must match desktop-vX.Y.Z-beta.N");
  }
  if (typeof manifest.commit_sha !== "string" || !SHA40.test(manifest.commit_sha)) {
    structuralErrors.push("commit_sha must be a full 40-character SHA");
  }
  if (manifest.source_branch !== "main") structuralErrors.push("source_branch must be main");
  if (!validTimestamp(manifest.generated_at)) structuralErrors.push("generated_at must be an ISO timestamp");
  if (typeof manifest.profile_id !== "string" || !manifest.profile_id) structuralErrors.push("profile_id is required");
  if (typeof manifest.profile_sha256 !== "string" || !SHA256.test(manifest.profile_sha256)) {
    structuralErrors.push("profile_sha256 must be a SHA-256 digest");
  }

  if (!Array.isArray(manifest.lockfiles) || manifest.lockfiles.length === 0) {
    structuralErrors.push("lockfiles must be a non-empty array");
  }
  const lockfilePaths = [];
  for (const [index, lockfile] of (manifest.lockfiles ?? []).entries()) {
    if (!lockfile || typeof lockfile.path !== "string" || !lockfile.path) {
      structuralErrors.push(`lockfiles[${index}].path is required`);
    } else {
      lockfilePaths.push(lockfile.path);
    }
    if (!lockfile || typeof lockfile.sha256 !== "string" || !SHA256.test(lockfile.sha256)) {
      structuralErrors.push(`lockfiles[${index}].sha256 must be a SHA-256 digest`);
    }
  }
  for (const path of duplicates(lockfilePaths)) structuralErrors.push(`duplicate lockfile path: ${path}`);

  if (!Array.isArray(manifest.selected_gates) || manifest.selected_gates.length === 0) {
    structuralErrors.push("selected_gates must be a non-empty array");
  }
  const selectedById = new Map();
  const selectedLabels = [];
  for (const [index, gate] of (manifest.selected_gates ?? []).entries()) {
    const prefix = `selected_gates[${index}]`;
    if (!gate || typeof gate.id !== "string" || !gate.id) structuralErrors.push(`${prefix}.id is required`);
    if (!gate || typeof gate.label !== "string" || !gate.label) structuralErrors.push(`${prefix}.label is required`);
    if (!gate || typeof gate.command !== "string" || !gate.command) structuralErrors.push(`${prefix}.command is required`);
    if (!Array.isArray(gate?.required_for) || gate.required_for.length === 0) {
      structuralErrors.push(`${prefix}.required_for is required`);
    }
    if (typeof gate?.manual_allowed !== "boolean") structuralErrors.push(`${prefix}.manual_allowed must be boolean`);
    if (gate?.id) {
      if (selectedById.has(gate.id)) structuralErrors.push(`duplicate selected gate id: ${gate.id}`);
      selectedById.set(gate.id, gate);
    }
    if (gate?.label) selectedLabels.push(gate.label);
  }
  for (const label of duplicates(selectedLabels)) structuralErrors.push(`duplicate selected gate label: ${label}`);

  if (requiredState && !profile) structuralErrors.push(`release profile is required for ${requiredState} validation`);
  if (profile) {
    if (manifest.profile_id !== profile.id) structuralErrors.push(`profile_id must be ${profile.id}`);
    const expected = (profile.gates ?? []).filter((gate) => gate.required_for?.includes(requiredState ?? manifest.release_state));
    if (!sameMembers([...selectedById.keys()], expected.map((gate) => gate.id))) {
      structuralErrors.push("selected gates do not exactly match the release profile");
    }
    for (const expectedGate of expected) {
      const selected = selectedById.get(expectedGate.id);
      if (!selected) continue;
      if (selected.label !== expectedGate.label) structuralErrors.push(`${expectedGate.id} label does not match release profile`);
      if (selected.command !== expectedGate.command) structuralErrors.push(`${expectedGate.label} command does not match release profile`);
      if (selected.manual_allowed !== expectedGate.manual_allowed) {
        structuralErrors.push(`${expectedGate.label} manual_allowed does not match release profile`);
      }
      if (!sameMembers(selected.required_for ?? [], expectedGate.required_for ?? [])) {
        structuralErrors.push(`${expectedGate.label} required_for does not match release profile`);
      }
    }
  }

  if (!manifest.gate_summary || typeof manifest.gate_summary !== "object" || Array.isArray(manifest.gate_summary)) {
    structuralErrors.push("gate_summary must be an object");
  }
  if (!Array.isArray(manifest.gates)) structuralErrors.push("gates must be an array");
  const resultsById = new Map();
  const resultLabels = [];
  let executedCount = 0;
  let failedExecutionCount = 0;
  let failedConditionCount = 0;
  let unknownConditionCount = 0;
  for (const [index, gate] of (manifest.gates ?? []).entries()) {
    const prefix = `gates[${index}]`;
    if (!gate || typeof gate.id !== "string" || !gate.id) structuralErrors.push(`${prefix}.id is required`);
    if (!gate || typeof gate.label !== "string" || !gate.label) structuralErrors.push(`${prefix}.label is required`);
    if (gate?.id) {
      if (resultsById.has(gate.id)) structuralErrors.push(`duplicate gate result id: ${gate.id}`);
      resultsById.set(gate.id, gate);
    }
    if (gate?.label) resultLabels.push(gate.label);
    if (!EXECUTION_STATES.has(gate?.execution_status)) structuralErrors.push(`${prefix}.execution_status is invalid`);
    if (!CONDITION_STATES.has(gate?.condition_status)) structuralErrors.push(`${prefix}.condition_status is invalid`);
    if (gate?.status !== gate?.condition_status) structuralErrors.push(`${prefix}.status must match condition_status`);
    if (gate?.evidence_sha256 !== undefined && !SHA256.test(String(gate.evidence_sha256))) {
      structuralErrors.push(`${prefix}.evidence_sha256 must be a SHA-256 digest`);
    }
    const selected = selectedById.get(gate?.id);
    if (!selected) {
      structuralErrors.push(`gate result ${gate?.id ?? index} was not selected`);
    } else {
      if (gate.label !== selected.label) structuralErrors.push(`${gate.id} result label does not match selected gate`);
      if (gate.command !== selected.command) structuralErrors.push(`${gate.label} result command does not match selected gate`);
      if (!sameMembers(gate.required_for ?? [], selected.required_for ?? [])) {
        structuralErrors.push(`${gate.label} result required_for does not match selected gate`);
      }
      if (gate.manual_allowed !== selected.manual_allowed) {
        structuralErrors.push(`${gate.label} result manual_allowed does not match selected gate`);
      }
    }
    if (gate?.execution_status === "completed" || gate?.execution_status === "execution_failed") executedCount += 1;
    if (gate?.execution_status === "execution_failed") failedExecutionCount += 1;
    if (gate?.condition_status === "failed") failedConditionCount += 1;
    if (gate?.condition_status === "unknown" || gate?.condition_status === "manual") unknownConditionCount += 1;
    if (requiredState && gate?.required_for?.includes(requiredState)) {
      if (gate.execution_status !== "completed" || gate.condition_status !== "passed") {
        stateErrors.push(`required gate ${gate.label ?? gate.id} is ${gate.execution_status}/${gate.condition_status}`);
      }
      if (gate.condition_status === "manual" && !gate.manual_allowed) {
        stateErrors.push(`required gate ${gate.label ?? gate.id} does not allow manual evidence`);
      }
    }
  }
  for (const label of duplicates(resultLabels)) structuralErrors.push(`duplicate gate result label: ${label}`);
  if (!sameMembers([...selectedById.keys()], [...resultsById.keys()])) {
    structuralErrors.push("selected-vs-executed gate results do not match");
  }
  const expectedSummary = {
    selected_count: manifest.selected_gates?.length ?? 0,
    executed_count: executedCount,
    failed_execution_count: failedExecutionCount,
    failed_condition_count: failedConditionCount,
    unknown_condition_count: unknownConditionCount,
  };
  for (const [field, expected] of Object.entries(expectedSummary)) {
    if (manifest.gate_summary?.[field] !== expected) {
      structuralErrors.push(`gate_summary.${field} must be ${expected}`);
    }
  }

  if (!Array.isArray(manifest.input_bindings)) structuralErrors.push("input_bindings must be an array");
  const bindingRoles = [];
  for (const [index, binding] of (manifest.input_bindings ?? []).entries()) {
    const prefix = `input_bindings[${index}]`;
    if (!binding || typeof binding.role !== "string" || !binding.role) structuralErrors.push(`${prefix}.role is required`);
    else bindingRoles.push(binding.role);
    if (!binding || typeof binding.path !== "string" || !binding.path) structuralErrors.push(`${prefix}.path is required`);
    if (!binding || typeof binding.sha256 !== "string" || !SHA256.test(binding.sha256)) {
      structuralErrors.push(`${prefix}.sha256 must be a SHA-256 digest`);
    }
    if (!binding || typeof binding.bound_commit_sha !== "string" || !SHA40.test(binding.bound_commit_sha)) {
      structuralErrors.push(`${prefix}.bound_commit_sha must be a full 40-character SHA`);
    } else if (binding.bound_commit_sha !== manifest.commit_sha) {
      structuralErrors.push(`${prefix}.bound_commit_sha does not match manifest commit_sha`);
    }
  }
  for (const role of duplicates(bindingRoles)) structuralErrors.push(`duplicate input binding role: ${role}`);
  for (const role of REQUIRED_INPUT_ROLES) {
    if (!bindingRoles.includes(role)) structuralErrors.push(`missing required input binding: ${role}`);
  }

  if (!Array.isArray(manifest.eval_evidence)) structuralErrors.push("eval_evidence must be an array");
  const evalRoles = [];
  for (const [index, evidence] of (manifest.eval_evidence ?? []).entries()) {
    const prefix = `eval_evidence[${index}]`;
    if (!evidence || typeof evidence.role !== "string" || !evidence.role) structuralErrors.push(`${prefix}.role is required`);
    else evalRoles.push(evidence.role);
    if (!evidence || typeof evidence.path !== "string" || !evidence.path) structuralErrors.push(`${prefix}.path is required`);
    if (!evidence || typeof evidence.sha256 !== "string" || !SHA256.test(evidence.sha256)) {
      structuralErrors.push(`${prefix}.sha256 must be a SHA-256 digest`);
    }
  }
  for (const role of ["representative_mock", "representative_real_forge"]) {
    if (!evalRoles.includes(role)) structuralErrors.push(`missing Eval evidence: ${role}`);
  }
  const realForge = (manifest.eval_evidence ?? []).find((evidence) => evidence.role === "representative_real_forge");
  if (requiredState === "R3" && realForge?.trust_result !== "trusted") {
    stateErrors.push("representative real-Forge evidence must have trust_result trusted");
  }

  for (const field of ["artifacts", "residual_risks"]) {
    if (!Array.isArray(manifest[field])) structuralErrors.push(`${field} must be an array`);
  }
  for (const field of ["signing", "notarization", "installation_smoke", "website"]) {
    const value = manifest[field];
    if (!value || typeof value !== "object" || Array.isArray(value) || typeof value.status !== "string") {
      structuralErrors.push(`${field}.status is required`);
    }
  }
  if (Object.hasOwn(manifest, "previous_release") && manifest.previous_release !== null) {
    if (typeof manifest.previous_release !== "string" || !manifest.previous_release) {
      structuralErrors.push("previous_release must be null or a non-empty string");
    }
  }
  const gitnexus = manifest.gitnexus;
  if (!gitnexus || typeof gitnexus !== "object" || Array.isArray(gitnexus)) {
    structuralErrors.push("gitnexus must be an object");
  } else {
    if (!new Set(["indexed", "fallback"]).has(gitnexus.mode)) structuralErrors.push("gitnexus.mode must be indexed or fallback");
    for (const field of ["current_commit", "indexed_commit"]) {
      if (typeof gitnexus[field] !== "string" || !SHA40.test(gitnexus[field])) {
        structuralErrors.push(`gitnexus.${field} must be a full 40-character SHA`);
      }
    }
    if (gitnexus.current_commit !== manifest.commit_sha) {
      structuralErrors.push("gitnexus.current_commit does not match manifest commit_sha");
    }
    if (typeof gitnexus.evidence_path !== "string" || !gitnexus.evidence_path) {
      structuralErrors.push("gitnexus.evidence_path is required");
    }
    if (typeof gitnexus.evidence_sha256 !== "string" || !SHA256.test(gitnexus.evidence_sha256)) {
      structuralErrors.push("gitnexus.evidence_sha256 must be a SHA-256 digest");
    }
    if (typeof gitnexus.impact_risk !== "string" || !gitnexus.impact_risk) {
      structuralErrors.push("gitnexus.impact_risk is required");
    }
  }

  const errors = [...structuralErrors, ...stateErrors];
  return { ok: errors.length === 0, errors, structuralErrors, stateErrors };
}

function readJson(path, label) {
  if (!path || !existsSync(path)) throw new Error(`${label} must point to an existing JSON file`);
  return JSON.parse(readFileSync(path, "utf8"));
}

function main(argv) {
  const manifestIndex = argv.indexOf("--manifest");
  const manifestPath = manifestIndex >= 0 ? argv[manifestIndex + 1] : null;
  const requiredIndex = argv.indexOf("--require-state");
  const requiredState = requiredIndex >= 0 ? argv[requiredIndex + 1] : null;
  const profileIndex = argv.indexOf("--release-profile");
  const profilePath = profileIndex >= 0 ? argv[profileIndex + 1] : null;
  let manifest;
  let profile = null;
  try {
    manifest = readJson(manifestPath, "--manifest");
    if (profilePath) profile = readJson(profilePath, "--release-profile");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 2;
  }
  const result = validateReleaseManifest(manifest, { requiredState, profile });
  console.log(JSON.stringify(result, null, 2));
  if (result.structuralErrors.length > 0) return 2;
  return result.ok ? 0 : 1;
}

if (process.argv[1] === new URL(import.meta.url).pathname) process.exitCode = main(process.argv.slice(2));
