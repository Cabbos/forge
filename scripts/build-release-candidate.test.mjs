import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import {
  buildReleaseCandidate,
  writeCandidateAtomic,
} from "./build-release-candidate.mjs";
import { validateReleaseManifest } from "./validate-release-manifest.mjs";

const repoRoot = new URL("..", import.meta.url).pathname;
const profilePath = join(repoRoot, "release/release-gates.v1.json");
const desktopEvidencePath = join(repoRoot, "release/evidence/merge-train/desktop-safety.json");
const evalEvidencePath = join(repoRoot, "release/evidence/merge-train/eval-trustworthiness.json");
const profile = JSON.parse(readFileSync(profilePath, "utf8"));
const COMMIT = "a".repeat(40);

test("root release scripts delegate to the shared candidate and validator helpers", () => {
  const rootPackage = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
  assert.equal(rootPackage.scripts["release:candidate"], "node scripts/build-release-candidate.mjs");
  assert.equal(
    rootPackage.scripts["release:validate"],
    "node scripts/validate-release-manifest.mjs --release-profile release/release-gates.v1.json",
  );
});

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
  return path;
}

function fixture(t) {
  const rootDir = mkdtempSync(join(tmpdir(), "forge-release-candidate-"));
  t.after(() => rmSync(rootDir, { recursive: true, force: true }));
  for (const path of [
    "apps/desktop/package-lock.json",
    "apps/desktop/src-tauri/Cargo.lock",
    "apps/eval-runner/uv.lock",
    "apps/website/package-lock.json",
  ]) {
    const absolute = join(rootDir, path);
    mkdirSync(dirname(absolute), { recursive: true });
    writeFileSync(absolute, `${path}\n`);
  }
  const selected = profile.gates.filter((gate) => gate.required_for.includes("R3"));
  const acceptance = {
    schemaVersion: 2,
    generatedAt: "2026-07-15T00:00:00.000Z",
    gateProfileId: profile.id,
    commitSha: COMMIT,
    status: "passed",
    selectedGateCount: selected.length,
    executedGateCount: selected.length,
    failedGateCount: 0,
    failedExecutionCount: 0,
    failedConditionCount: 0,
    unknownConditionCount: 0,
    gates: selected.map((gate) => ({
      label: gate.label,
      command: gate.command,
      status: "passed",
      exitCode: 0,
      executionStatus: "completed",
      conditionStatus: "passed",
    })),
  };
  const representative = (evidenceType) => ({
    schema_version: 1,
    evidence_type: evidenceType,
    producer_commit: COMMIT,
    run_id: `${evidenceType}-run`,
    execution_status: "completed",
    condition_status: "passed",
    trust_result: "trusted",
  });
  const paths = {
    acceptanceJson: writeJson(join(rootDir, "inputs/acceptance.json"), acceptance),
    desktopSafetyEvidence: desktopEvidencePath,
    evalTrustEvidence: evalEvidencePath,
    gitnexusEvidence: writeJson(join(rootDir, "inputs/gitnexus.json"), {
      schema_version: 1,
      mode: "indexed",
      current_commit: COMMIT,
      indexed_commit: COMMIT,
      impact_risk: "LOW",
      refresh_error: null,
      residual_risks: [],
    }),
    mockEvidence: writeJson(
      join(rootDir, "inputs/mock.json"),
      representative("representative_mock"),
    ),
    realForgeEvidence: writeJson(
      join(rootDir, "inputs/real.json"),
      representative("representative_real_forge"),
    ),
  };
  const git = {
    commit: COMMIT,
    branch: "main",
    clean: true,
    reachableFromMain: true,
    isAncestor: () => true,
  };
  return {
    rootDir,
    tag: "desktop-v0.1.0-beta.1",
    releaseProfile: profilePath,
    requiredState: "R3",
    generatedAt: "2026-07-15T00:00:00.000Z",
    git,
    ...paths,
  };
}

test("builds a validated local structural fixture without declaring R3", (t) => {
  const options = fixture(t);
  const manifest = buildReleaseCandidate({ ...options, releaseState: "R0" });
  assert.equal(manifest.release_state, "R0");
  assert.equal(manifest.commit_sha, COMMIT);
  assert.equal(manifest.gates.length, profile.gates.filter((gate) => gate.required_for.includes("R3")).length);
  assert.deepEqual(validateReleaseManifest(manifest), {
    ok: true,
    errors: [],
    structuralErrors: [],
    stateErrors: [],
  });
});

test("builds a complete R3 manifest when every explicit input is valid", (t) => {
  const options = fixture(t);
  const manifest = buildReleaseCandidate(options);
  assert.deepEqual(
    validateReleaseManifest(manifest, { requiredState: "R3", profile }),
    { ok: true, errors: [], structuralErrors: [], stateErrors: [] },
  );
  assert.deepEqual(
    manifest.input_bindings.map((binding) => binding.role),
    [
      "acceptance_results",
      "desktop_safety",
      "eval_trustworthiness",
      "gitnexus",
      "representative_mock",
      "representative_real_forge",
    ],
  );
});

test("missing owner evidence writes no candidate", (t) => {
  const options = fixture(t);
  options.desktopSafetyEvidence = join(options.rootDir, "missing-desktop.json");
  const outputRoot = join(options.rootDir, "release/evidence");
  assert.throws(() => {
    const manifest = buildReleaseCandidate(options);
    writeCandidateAtomic(manifest, { outputRoot, tag: options.tag });
  }, /desktop-safety-evidence/);
  assert.equal(
    (() => {
      try {
        readFileSync(join(outputRoot, options.tag, "candidate-manifest.json"));
        return true;
      } catch {
        return false;
      }
    })(),
    false,
  );
});

test("rejects dirty worktrees before reading candidate inputs", (t) => {
  const options = fixture(t);
  options.git.clean = false;
  assert.throws(() => buildReleaseCandidate(options), /clean Git tree/);
});

test("rejects stale selected gate counts and acceptance from another commit", (t) => {
  const options = fixture(t);
  const acceptance = JSON.parse(readFileSync(options.acceptanceJson, "utf8"));
  acceptance.selectedGateCount -= 1;
  writeJson(options.acceptanceJson, acceptance);
  assert.throws(() => buildReleaseCandidate(options), /selected gate count is stale/);

  acceptance.selectedGateCount += 1;
  acceptance.commitSha = "f".repeat(40);
  writeJson(options.acceptanceJson, acceptance);
  assert.throws(() => buildReleaseCandidate(options), /commitSha does not match/);
});

test("rejects a feature branch, an unreachable commit, and untrusted real Forge", (t) => {
  const options = fixture(t);
  options.git.branch = "feature";
  assert.throws(() => buildReleaseCandidate(options), /source branch main/);
  options.git.branch = "main";
  options.git.reachableFromMain = false;
  assert.throws(() => buildReleaseCandidate(options), /reachable from main/);
  options.git.reachableFromMain = true;
  const real = JSON.parse(readFileSync(options.realForgeEvidence, "utf8"));
  real.trust_result = "unknown";
  writeJson(options.realForgeEvidence, real);
  assert.throws(() => buildReleaseCandidate(options), /trust_result trusted/);
});

test("writes atomically and only replaces the same R3 commit", (t) => {
  const options = fixture(t);
  const manifest = buildReleaseCandidate(options);
  const outputRoot = join(options.rootDir, "release/evidence");
  const outputPath = writeCandidateAtomic(manifest, { outputRoot, tag: options.tag });
  assert.equal(JSON.parse(readFileSync(outputPath, "utf8")).commit_sha, COMMIT);
  assert.throws(
    () => writeCandidateAtomic(manifest, { outputRoot, tag: options.tag }),
    /candidate already exists/,
  );
  assert.equal(
    writeCandidateAtomic(manifest, {
      outputRoot,
      tag: options.tag,
      replaceExisting: true,
    }),
    outputPath,
  );
  assert.throws(
    () =>
      writeCandidateAtomic(
        { ...manifest, commit_sha: "f".repeat(40) },
        { outputRoot, tag: options.tag, replaceExisting: true },
      ),
    /same R3 candidate commit/,
  );
});
