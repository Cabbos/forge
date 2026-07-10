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
  "release candidate manifest validation",
]);

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
