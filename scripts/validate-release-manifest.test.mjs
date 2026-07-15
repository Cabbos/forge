import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { validateReleaseManifest } from "./validate-release-manifest.mjs";

const profile = JSON.parse(readFileSync(new URL("../release/release-gates.v1.json", import.meta.url), "utf8"));
const COMMIT = "a".repeat(40);
const DIGEST = "b".repeat(64);

function selectedR3Gates() {
  return profile.gates
    .filter((gate) => gate.required_for.includes("R3"))
    .map((gate) => ({
      id: gate.id,
      label: gate.label,
      command: gate.command,
      required_for: gate.required_for,
      manual_allowed: gate.manual_allowed,
    }));
}

function validManifest() {
  const selectedGates = selectedR3Gates();
  return {
    schema_version: 1,
    release_state: "R3",
    version: "desktop-v0.1.0-beta.1",
    commit_sha: COMMIT,
    source_branch: "main",
    generated_at: "2026-07-10T00:00:00.000Z",
    profile_id: profile.id,
    profile_sha256: DIGEST,
    lockfiles: [{ path: "apps/desktop/package-lock.json", sha256: DIGEST }],
    selected_gates: selectedGates,
    gate_summary: {
      selected_count: selectedGates.length,
      executed_count: selectedGates.length,
      failed_execution_count: 0,
      failed_condition_count: 0,
      unknown_condition_count: 0,
    },
    gates: selectedGates.map((gate) => ({
      ...gate,
      execution_status: "completed",
      condition_status: "passed",
      status: "passed",
      exit_code: 0,
      evidence_sha256: DIGEST,
    })),
    input_bindings: [
      "acceptance_results",
      "desktop_safety",
      "eval_trustworthiness",
      "gitnexus",
      "representative_mock",
      "representative_real_forge",
    ].map((role) => ({
      role,
      path: `${role}.json`,
      sha256: DIGEST,
      bound_commit_sha: COMMIT,
    })),
    eval_evidence: [
      { role: "representative_mock", path: "mock.json", sha256: DIGEST, trust_result: "trusted" },
      { role: "representative_real_forge", path: "real.json", sha256: DIGEST, trust_result: "trusted" },
    ],
    artifacts: [],
    signing: { status: "not_started" },
    notarization: { status: "not_started" },
    installation_smoke: { status: "not_started" },
    website: { status: "not_started" },
    previous_release: null,
    gitnexus: {
      mode: "indexed",
      current_commit: COMMIT,
      indexed_commit: COMMIT,
      impact_risk: "LOW",
      evidence_path: "gitnexus.json",
      evidence_sha256: DIGEST,
    },
    residual_risks: [],
  };
}

test("accepts a complete profile-bound R3 manifest", () => {
  assert.deepEqual(
    validateReleaseManifest(validManifest(), { requiredState: "R3", profile }),
    { ok: true, errors: [], structuralErrors: [], stateErrors: [] },
  );
});

test("rejects missing required manifest fields and lockfile hashes structurally", () => {
  const manifest = validManifest();
  delete manifest.previous_release;
  delete manifest.gitnexus;
  manifest.lockfiles[0].sha256 = "";
  const result = validateReleaseManifest(manifest, { requiredState: "R3", profile });
  assert.equal(result.ok, false);
  assert.match(result.structuralErrors.join("\n"), /previous_release/);
  assert.match(result.structuralErrors.join("\n"), /gitnexus/);
  assert.match(result.structuralErrors.join("\n"), /lockfiles\[0\]\.sha256/);
});

test("rejects duplicate gate labels and selected-vs-executed mismatch", () => {
  const manifest = validManifest();
  manifest.selected_gates.push({ ...manifest.selected_gates[0], id: "different-id" });
  manifest.gates.pop();
  const result = validateReleaseManifest(manifest, { requiredState: "R3", profile });
  assert.equal(result.ok, false);
  assert.match(result.structuralErrors.join("\n"), /duplicate selected gate label/);
  assert.match(result.structuralErrors.join("\n"), /selected-vs-executed/);
});

test("rejects failed or unknown required gates as release-state failures", () => {
  const manifest = validManifest();
  manifest.gates[0].condition_status = "unknown";
  manifest.gates[0].status = "unknown";
  manifest.gates[1].condition_status = "failed";
  manifest.gates[1].status = "failed";
  manifest.gate_summary.failed_condition_count = 1;
  manifest.gate_summary.unknown_condition_count = 1;
  const result = validateReleaseManifest(manifest, { requiredState: "R3", profile });
  assert.equal(result.ok, false);
  assert.equal(result.structuralErrors.length, 0);
  assert.match(result.stateErrors.join("\n"), /unknown/);
  assert.match(result.stateErrors.join("\n"), /failed/);
});

test("rejects a mismatched bound commit and incomplete input bindings", () => {
  const manifest = validManifest();
  manifest.input_bindings[0].bound_commit_sha = "f".repeat(40);
  manifest.input_bindings.pop();
  const result = validateReleaseManifest(manifest, { requiredState: "R3", profile });
  assert.equal(result.ok, false);
  assert.match(result.structuralErrors.join("\n"), /bound_commit_sha does not match/);
  assert.match(result.structuralErrors.join("\n"), /representative_real_forge/);
});

test("rejects profile selection drift and an untrusted real-Forge result", () => {
  const manifest = validManifest();
  manifest.selected_gates[0].command = "echo drift";
  manifest.eval_evidence.find((item) => item.role === "representative_real_forge").trust_result = "unknown";
  const result = validateReleaseManifest(manifest, { requiredState: "R3", profile });
  assert.equal(result.ok, false);
  assert.match(result.structuralErrors.join("\n"), /command does not match release profile/);
  assert.match(result.stateErrors.join("\n"), /real-Forge.*trusted/);
});

test("manifest CLI returns 2 for structure errors and 1 for release-state failures", (t) => {
  const tempDir = mkdtempSync(join(tmpdir(), "forge-manifest-cli-"));
  t.after(() => rmSync(tempDir, { recursive: true, force: true }));
  const manifestPath = join(tempDir, "candidate.json");
  const scriptPath = new URL("validate-release-manifest.mjs", import.meta.url).pathname;
  const args = [
    scriptPath,
    "--manifest",
    manifestPath,
    "--release-profile",
    new URL("../release/release-gates.v1.json", import.meta.url).pathname,
    "--require-state",
    "R3",
  ];

  const malformed = validManifest();
  malformed.commit_sha = "short";
  writeFileSync(manifestPath, JSON.stringify(malformed));
  assert.equal(spawnSync(process.execPath, args, { encoding: "utf8" }).status, 2);

  const failed = validManifest();
  failed.gates[0].condition_status = "failed";
  failed.gates[0].status = "failed";
  failed.gate_summary.failed_condition_count = 1;
  writeFileSync(manifestPath, JSON.stringify(failed));
  assert.equal(spawnSync(process.execPath, args, { encoding: "utf8" }).status, 1);
});
