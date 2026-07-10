import { existsSync, readFileSync } from "node:fs";

const SHA256 = /^[0-9a-f]{64}$/i;
const SHA40 = /^[0-9a-f]{40}$/i;
const RELEASE_STATES = new Set(["R0", "R1", "R2", "R3", "R4"]);
const EXECUTION_STATES = new Set(["not_started", "running", "completed", "execution_failed"]);
const CONDITION_STATES = new Set(["passed", "failed", "manual", "unknown"]);

export function validateReleaseManifest(manifest, { requiredState = null } = {}) {
  const errors = [];
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return { ok: false, errors: ["manifest must be an object"] };
  }
  if (manifest.schema_version !== 1) errors.push("schema_version must be 1");
  if (!RELEASE_STATES.has(manifest.release_state)) errors.push("release_state must be R0, R1, R2, R3, or R4");
  if (requiredState && manifest.release_state !== requiredState) {
    errors.push(`release_state must be ${requiredState}`);
  }
  if (typeof manifest.version !== "string" || !manifest.version) errors.push("version is required");
  if (typeof manifest.commit_sha !== "string" || !SHA40.test(manifest.commit_sha)) errors.push("commit_sha must be a full 40-character SHA");
  if (manifest.source_branch !== "main") errors.push("source_branch must be main");
  if (typeof manifest.profile_id !== "string" || !manifest.profile_id) errors.push("profile_id is required");
  if (!Array.isArray(manifest.lockfiles) || manifest.lockfiles.length === 0) errors.push("lockfiles must be non-empty");
  for (const [index, lockfile] of (manifest.lockfiles ?? []).entries()) {
    if (!lockfile || typeof lockfile.path !== "string" || !lockfile.path) errors.push(`lockfiles[${index}].path is required`);
    if (!lockfile || typeof lockfile.sha256 !== "string" || !SHA256.test(lockfile.sha256)) errors.push(`lockfiles[${index}].sha256 must be a SHA-256 digest`);
  }
  if (!Array.isArray(manifest.gates)) errors.push("gates must be an array");
  const seenGateIds = new Set();
  for (const [index, gate] of (manifest.gates ?? []).entries()) {
    if (!gate || typeof gate.id !== "string" || !gate.id) errors.push(`gates[${index}].id is required`);
    if (seenGateIds.has(gate?.id)) errors.push(`duplicate gate id: ${gate.id}`);
    if (gate?.id) seenGateIds.add(gate.id);
    if (!EXECUTION_STATES.has(gate?.execution_status)) errors.push(`gates[${index}].execution_status is invalid`);
    if (!CONDITION_STATES.has(gate?.condition_status)) errors.push(`gates[${index}].condition_status is invalid`);
    if (gate?.execution_status !== "completed" || gate?.condition_status !== "passed") {
      if (requiredState && Array.isArray(gate?.required_for) && gate.required_for.includes(requiredState)) {
        errors.push(`required gate ${gate.id} is ${gate.execution_status}/${gate.condition_status}`);
      }
    }
    if (gate?.evidence_sha256 !== undefined && !SHA256.test(String(gate.evidence_sha256))) {
      errors.push(`gates[${index}].evidence_sha256 must be a SHA-256 digest`);
    }
  }
  for (const field of ["eval_evidence", "artifacts", "residual_risks"]) {
    if (!Array.isArray(manifest[field])) errors.push(`${field} must be an array`);
  }
  if (typeof manifest.generated_at !== "string" || Number.isNaN(Date.parse(manifest.generated_at))) errors.push("generated_at must be an ISO timestamp");
  return { ok: errors.length === 0, errors };
}

function main(argv) {
  const manifestIndex = argv.indexOf("--manifest");
  const manifestPath = manifestIndex >= 0 ? argv[manifestIndex + 1] : null;
  const requiredIndex = argv.indexOf("--require-state");
  const requiredState = requiredIndex >= 0 ? argv[requiredIndex + 1] : null;
  if (!manifestPath || !existsSync(manifestPath)) {
    console.error("--manifest must point to an existing JSON file");
    return 2;
  }
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 2;
  }
  const result = validateReleaseManifest(manifest, { requiredState });
  console.log(JSON.stringify(result, null, 2));
  return result.ok ? 0 : 1;
}

if (process.argv[1] === new URL(import.meta.url).pathname) process.exitCode = main(process.argv.slice(2));
