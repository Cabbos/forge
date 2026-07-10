import assert from "node:assert/strict";
import test from "node:test";
import { validateReleaseManifest } from "./validate-release-manifest.mjs";

function validManifest() {
  return {
    schema_version: 1,
    release_state: "R3",
    version: "desktop-v0.1.0-beta.1",
    commit_sha: "a".repeat(40),
    source_branch: "main",
    lockfiles: [{ path: "apps/desktop/package-lock.json", sha256: "b".repeat(64) }],
    profile_id: "public-beta-r3-v1",
    gates: [
      {
        id: "desktop-build",
        required_for: ["R3"],
        execution_status: "completed",
        condition_status: "passed",
        evidence_sha256: "c".repeat(64),
      },
    ],
    eval_evidence: [{ path: "eval-report.json", sha256: "d".repeat(64) }],
    artifacts: [],
    signing: {},
    notarization: {},
    installation_smoke: {},
    website: {},
    residual_risks: [],
    generated_at: "2026-07-10T00:00:00.000Z",
  };
}

test("accepts a complete R3 manifest", () => {
  assert.deepEqual(validateReleaseManifest(validManifest(), { requiredState: "R3" }), {
    ok: true,
    errors: [],
  });
});

test("rejects malformed identity and missing required evidence", () => {
  const manifest = validManifest();
  manifest.commit_sha = "short";
  manifest.gates[0].condition_status = "unknown";
  const result = validateReleaseManifest(manifest, { requiredState: "R3" });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /commit_sha/);
  assert.match(result.errors.join("\n"), /unknown/);
});

test("rejects duplicate gate ids and incomplete execution", () => {
  const manifest = validManifest();
  manifest.gates.push({ ...manifest.gates[0] });
  manifest.gates[1].execution_status = "not_started";
  const result = validateReleaseManifest(manifest, { requiredState: "R3" });
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /duplicate gate id/);
  assert.match(result.errors.join("\n"), /not_started/);
});
