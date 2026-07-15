import assert from "node:assert/strict";
import test from "node:test";
import { buildRepresentativeEvidence } from "./build-representative-evidence.mjs";

const COMMIT = "a".repeat(40);
const DIGEST = "b".repeat(64);

function artifact(provider = "forge") {
  return {
    report: {
      total_tasks: 1,
      success_rate: 1,
      verification_pass_rate: 1,
      scope_violation_rate: 0,
      trust_result: { status: "trusted", trusted: true, blockers: [] },
    },
    traces: [{ task_id: "forge-session-capitalize", provider }],
  };
}

test("builds trusted representative real-Forge evidence", () => {
  const evidence = buildRepresentativeEvidence({
    sourcePath: "real-backtest.json",
    source: artifact(),
    sourceSha256: DIGEST,
    evidenceType: "representative_real_forge",
    commit: COMMIT,
  });
  assert.equal(evidence.trust_result, "trusted");
  assert.equal(evidence.condition_status, "passed");
  assert.equal(evidence.source_artifact.sha256, DIGEST);
});

test("rejects provider drift and untrusted or incomplete reports", () => {
  assert.throws(
    () =>
      buildRepresentativeEvidence({
        sourcePath: "real.json",
        source: artifact("mock"),
        sourceSha256: DIGEST,
        evidenceType: "representative_real_forge",
        commit: COMMIT,
      }),
    /provider forge/,
  );
  const untrusted = artifact();
  untrusted.report.trust_result = { status: "untrusted", trusted: false, blockers: ["x"] };
  assert.throws(
    () =>
      buildRepresentativeEvidence({
        sourcePath: "real.json",
        source: untrusted,
        sourceSha256: DIGEST,
        evidenceType: "representative_real_forge",
        commit: COMMIT,
      }),
    /trust_result must be trusted/,
  );
  const failed = artifact();
  failed.report.success_rate = 0;
  assert.throws(
    () =>
      buildRepresentativeEvidence({
        sourcePath: "real.json",
        source: failed,
        sourceSha256: DIGEST,
        evidenceType: "representative_real_forge",
        commit: COMMIT,
      }),
    /must pass success/,
  );
});
