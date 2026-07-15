import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { validateReleaseGateProfile, validateReleaseHandoff } from "./validate-release-gate-profile.mjs";
import { validateReleaseManifest } from "./validate-release-manifest.mjs";

const TAG_PATTERN = /^desktop-v\d+\.\d+\.\d+-beta\.\d+$/;
const SHA40 = /^[0-9a-f]{40}$/i;
const SHA256 = /^[0-9a-f]{64}$/i;
const LOCKFILES = Object.freeze([
  "apps/desktop/package-lock.json",
  "apps/desktop/src-tauri/Cargo.lock",
  "apps/eval-runner/uv.lock",
  "apps/website/package-lock.json",
]);

function digest(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function readInput(path, label) {
  if (!path || !existsSync(path)) throw new Error(`${label} must point to an existing JSON file`);
  const raw = readFileSync(path);
  let value;
  try {
    value = JSON.parse(raw.toString("utf8"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  return { path, raw, sha256: digest(raw), value };
}

function assertPassedEvidence(input, role, git) {
  const evidence = input.value;
  if (evidence.schema_version !== 1) throw new Error(`${role} schema_version must be 1`);
  if (evidence.evidence_type !== role) throw new Error(`${role} evidence_type must be ${role}`);
  if (!SHA40.test(String(evidence.producer_commit ?? ""))) {
    throw new Error(`${role} producer_commit must be a full 40-character SHA`);
  }
  if (!git.isAncestor(evidence.producer_commit, git.commit)) {
    throw new Error(`${role} producer_commit is not an ancestor of candidate commit`);
  }
  if (evidence.execution_status !== "completed" || evidence.condition_status !== "passed") {
    throw new Error(`${role} evidence is not completed/passed`);
  }
  if (role === "representative_real_forge" && evidence.trust_result !== "trusted") {
    throw new Error("representative real-Forge evidence must have trust_result trusted");
  }
  return evidence;
}

function validateAcceptance(acceptance, profile, requiredState, commit) {
  const expectedGates = profile.gates.filter((gate) => gate.required_for.includes(requiredState));
  if (acceptance.schemaVersion !== 2) throw new Error("acceptance results schemaVersion must be 2");
  if (acceptance.gateProfileId !== profile.id) throw new Error("acceptance results gateProfileId does not match release profile");
  if (acceptance.commitSha !== commit) throw new Error("acceptance results commitSha does not match candidate commit");
  if (acceptance.status !== "passed") throw new Error("acceptance results status must be passed");
  if (acceptance.selectedGateCount !== expectedGates.length) {
    throw new Error(`acceptance selected gate count is stale: expected ${expectedGates.length}, got ${acceptance.selectedGateCount}`);
  }
  if (acceptance.executedGateCount !== acceptance.selectedGateCount) {
    throw new Error("acceptance selected and executed gate counts do not match");
  }
  for (const field of [
    "failedGateCount",
    "failedExecutionCount",
    "failedConditionCount",
    "unknownConditionCount",
  ]) {
    if (acceptance[field] !== 0) throw new Error(`acceptance ${field} must be 0`);
  }
  if (!Array.isArray(acceptance.gates) || acceptance.gates.length !== expectedGates.length) {
    throw new Error("acceptance gate result count does not match selected profile gates");
  }
  const results = new Map();
  for (const gate of acceptance.gates) {
    if (results.has(gate.label)) throw new Error(`duplicate acceptance gate label: ${gate.label}`);
    results.set(gate.label, gate);
  }
  for (const expected of expectedGates) {
    const result = results.get(expected.label);
    if (!result) throw new Error(`missing acceptance gate: ${expected.label}`);
    if (result.command !== expected.command) throw new Error(`acceptance command drift for ${expected.label}`);
    if (result.executionStatus !== "completed" || result.conditionStatus !== "passed") {
      throw new Error(`acceptance gate ${expected.label} is not completed/passed`);
    }
    if (result.status !== result.conditionStatus || result.exitCode !== 0) {
      throw new Error(`acceptance gate ${expected.label} has inconsistent compatibility status`);
    }
  }
  return expectedGates;
}

function validateGitNexusEvidence(input, commit) {
  const evidence = input.value;
  if (evidence.schema_version !== 1) throw new Error("GitNexus evidence schema_version must be 1");
  if (!new Set(["indexed", "fallback"]).has(evidence.mode)) throw new Error("GitNexus evidence mode must be indexed or fallback");
  if (evidence.current_commit !== commit) throw new Error("GitNexus evidence current_commit does not match candidate commit");
  if (!SHA40.test(String(evidence.indexed_commit ?? ""))) throw new Error("GitNexus indexed_commit must be a full SHA");
  if (typeof evidence.impact_risk !== "string" || !evidence.impact_risk) throw new Error("GitNexus impact_risk is required");
  if (evidence.mode === "fallback" && !new Set(["HIGH", "CRITICAL"]).has(evidence.impact_risk)) {
    throw new Error("GitNexus fallback evidence must retain HIGH or CRITICAL residual risk");
  }
  return evidence;
}

function binding(role, input, commit, producerCommit = undefined) {
  const result = {
    role,
    path: input.path,
    sha256: input.sha256,
    bound_commit_sha: commit,
  };
  if (producerCommit) result.producer_commit = producerCommit;
  return result;
}

export function buildReleaseCandidate({
  rootDir,
  tag,
  releaseProfile,
  requiredState = "R3",
  acceptanceJson,
  desktopSafetyEvidence,
  evalTrustEvidence,
  gitnexusEvidence,
  mockEvidence,
  realForgeEvidence,
  git,
  generatedAt = new Date().toISOString(),
  releaseState = "R3",
}) {
  if (!TAG_PATTERN.test(String(tag ?? ""))) throw new Error("--tag must match desktop-vX.Y.Z-beta.N");
  if (requiredState !== "R3") throw new Error("candidate generation only supports --require-state R3");
  if (!git?.clean) throw new Error("candidate generation requires a clean Git tree");
  if (!SHA40.test(String(git.commit ?? ""))) throw new Error("candidate commit must be a full 40-character SHA");
  if (git.branch !== "main") throw new Error("candidate generation requires source branch main");
  if (!git.reachableFromMain) throw new Error("candidate commit must be reachable from main");

  const profileInput = readInput(releaseProfile, "--release-profile");
  const profileResult = validateReleaseGateProfile(profileInput.value, { requiredState });
  if (!profileResult.ok) throw new Error(`invalid release profile: ${profileResult.errors.join("; ")}`);
  const acceptanceInput = readInput(acceptanceJson, "--acceptance-json");
  const desktopInput = readInput(desktopSafetyEvidence, "--desktop-safety-evidence");
  const evalInput = readInput(evalTrustEvidence, "--eval-trust-evidence");
  const gitnexusInput = readInput(gitnexusEvidence, "--gitnexus-evidence");
  const mockInput = readInput(mockEvidence, "--mock-evidence");
  const realInput = readInput(realForgeEvidence, "--real-forge-evidence");

  const selectedProfileGates = validateAcceptance(
    acceptanceInput.value,
    profileInput.value,
    requiredState,
    git.commit,
  );
  for (const [state, input, label] of [
    ["R1", desktopInput, "Desktop Safety"],
    ["R2", evalInput, "Eval Trustworthiness"],
  ]) {
    const result = validateReleaseHandoff(profileInput.value, input.value, { requiredState: state });
    if (!result.ok) throw new Error(`${label} handoff is invalid: ${result.errors.join("; ")}`);
    if (!git.isAncestor(input.value.producer_commit, git.commit)) {
      throw new Error(`${label} producer_commit is not an ancestor of candidate commit`);
    }
  }
  const mock = assertPassedEvidence(mockInput, "representative_mock", git);
  const real = assertPassedEvidence(realInput, "representative_real_forge", git);
  const gitnexus = validateGitNexusEvidence(gitnexusInput, git.commit);

  const lockfiles = LOCKFILES.map((path) => {
    const absolutePath = join(rootDir, path);
    if (!existsSync(absolutePath)) throw new Error(`required lockfile is missing: ${path}`);
    return { path, sha256: digest(readFileSync(absolutePath)) };
  });
  const selectedGates = selectedProfileGates.map((gate) => ({
    id: gate.id,
    label: gate.label,
    command: gate.command,
    required_for: gate.required_for,
    manual_allowed: gate.manual_allowed,
  }));
  const acceptanceByLabel = new Map(acceptanceInput.value.gates.map((gate) => [gate.label, gate]));
  const gates = selectedGates.map((selected) => {
    const result = acceptanceByLabel.get(selected.label);
    return {
      ...selected,
      execution_status: result.executionStatus,
      condition_status: result.conditionStatus,
      status: result.status,
      exit_code: result.exitCode,
      evidence_sha256: acceptanceInput.sha256,
    };
  });
  const manifest = {
    schema_version: 1,
    release_state: releaseState,
    version: tag,
    commit_sha: git.commit,
    source_branch: "main",
    generated_at: generatedAt,
    profile_id: profileInput.value.id,
    profile_sha256: profileInput.sha256,
    lockfiles,
    selected_gates: selectedGates,
    gate_summary: {
      selected_count: acceptanceInput.value.selectedGateCount,
      executed_count: acceptanceInput.value.executedGateCount,
      failed_execution_count: acceptanceInput.value.failedExecutionCount,
      failed_condition_count: acceptanceInput.value.failedConditionCount,
      unknown_condition_count: acceptanceInput.value.unknownConditionCount,
    },
    gates,
    input_bindings: [
      binding("acceptance_results", acceptanceInput, git.commit),
      binding("desktop_safety", desktopInput, git.commit, desktopInput.value.producer_commit),
      binding("eval_trustworthiness", evalInput, git.commit, evalInput.value.producer_commit),
      binding("gitnexus", gitnexusInput, git.commit),
      binding("representative_mock", mockInput, git.commit, mock.producer_commit),
      binding("representative_real_forge", realInput, git.commit, real.producer_commit),
    ],
    eval_evidence: [
      {
        role: "representative_mock",
        path: mockInput.path,
        sha256: mockInput.sha256,
        trust_result: mock.trust_result,
      },
      {
        role: "representative_real_forge",
        path: realInput.path,
        sha256: realInput.sha256,
        trust_result: real.trust_result,
      },
    ],
    artifacts: [],
    signing: { status: "not_started" },
    notarization: { status: "not_started" },
    installation_smoke: { status: "not_started" },
    website: { status: "not_started" },
    previous_release: null,
    gitnexus: {
      mode: gitnexus.mode,
      current_commit: gitnexus.current_commit,
      indexed_commit: gitnexus.indexed_commit,
      impact_risk: gitnexus.impact_risk,
      evidence_path: gitnexusInput.path,
      evidence_sha256: gitnexusInput.sha256,
      refresh_error: gitnexus.refresh_error ?? null,
    },
    residual_risks: Array.isArray(gitnexus.residual_risks) ? gitnexus.residual_risks : [],
  };
  const validation = validateReleaseManifest(manifest, {
    requiredState: releaseState === "R3" ? "R3" : null,
    profile: releaseState === "R3" ? profileInput.value : null,
  });
  if (!validation.ok) throw new Error(`candidate manifest is invalid: ${validation.errors.join("; ")}`);
  return manifest;
}

export function writeCandidateAtomic(manifest, { outputRoot, tag, replaceExisting = false }) {
  const outputPath = join(outputRoot, tag, "candidate-manifest.json");
  if (existsSync(outputPath)) {
    if (!replaceExisting) throw new Error(`candidate already exists: ${outputPath}`);
    const existing = JSON.parse(readFileSync(outputPath, "utf8"));
    if (existing.release_state !== "R3" || existing.commit_sha !== manifest.commit_sha) {
      throw new Error("--replace-existing only permits the same R3 candidate commit");
    }
  }
  mkdirSync(dirname(outputPath), { recursive: true });
  const tempPath = `${outputPath}.tmp-${process.pid}-${Date.now()}`;
  try {
    writeFileSync(tempPath, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
    renameSync(tempPath, outputPath);
  } catch (error) {
    if (existsSync(tempPath)) unlinkSync(tempPath);
    throw error;
  }
  return outputPath;
}

function gitOutput(rootDir, args) {
  return execFileSync("git", args, { cwd: rootDir, encoding: "utf8" }).trim();
}

export function readGitState(rootDir) {
  const commit = gitOutput(rootDir, ["rev-parse", "HEAD"]);
  const branch = gitOutput(rootDir, ["branch", "--show-current"]);
  const clean = gitOutput(rootDir, ["status", "--porcelain"]) === "";
  const reachable = spawnSync("git", ["merge-base", "--is-ancestor", commit, "main"], {
    cwd: rootDir,
    stdio: "ignore",
  });
  return {
    commit,
    branch,
    clean,
    reachableFromMain: reachable.status === 0,
    isAncestor(ancestor, descendant) {
      return spawnSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], {
        cwd: rootDir,
        stdio: "ignore",
      }).status === 0;
    },
  };
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : null;
}

function main(argv) {
  const rootDir = resolve(option(argv, "--root") ?? new URL("..", import.meta.url).pathname);
  const tag = option(argv, "--tag");
  const replaceExisting = argv.includes("--replace-existing");
  try {
    const manifest = buildReleaseCandidate({
      rootDir,
      tag,
      releaseProfile: resolve(rootDir, option(argv, "--release-profile") ?? ""),
      requiredState: option(argv, "--require-state"),
      acceptanceJson: resolve(rootDir, option(argv, "--acceptance-json") ?? ""),
      desktopSafetyEvidence: resolve(rootDir, option(argv, "--desktop-safety-evidence") ?? ""),
      evalTrustEvidence: resolve(rootDir, option(argv, "--eval-trust-evidence") ?? ""),
      gitnexusEvidence: resolve(rootDir, option(argv, "--gitnexus-evidence") ?? ""),
      mockEvidence: resolve(rootDir, option(argv, "--mock-evidence") ?? ""),
      realForgeEvidence: resolve(rootDir, option(argv, "--real-forge-evidence") ?? ""),
      git: readGitState(rootDir),
    });
    const outputPath = writeCandidateAtomic(manifest, {
      outputRoot: resolve(rootDir, option(argv, "--output-root") ?? "release/evidence"),
      tag,
      replaceExisting,
    });
    console.log(
      JSON.stringify(
        {
          path: relative(rootDir, outputPath),
          commit_sha: manifest.commit_sha,
          sha256: digest(readFileSync(outputPath)),
        },
        null,
        2,
      ),
    );
    return 0;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 1;
  }
}

if (process.argv[1] === new URL(import.meta.url).pathname) process.exitCode = main(process.argv.slice(2));
