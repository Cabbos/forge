import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const baselineUrl = new URL(
  "../release/evidence/2026-07-10-r0-baseline.json",
  import.meta.url,
);

test("public beta R0 baseline is immutable and complete", () => {
  const baseline = JSON.parse(readFileSync(baselineUrl, "utf8"));

  assert.equal(baseline.schema_version, 1);
  assert.equal(baseline.baseline_id, "public-beta-r0-2026-07-10");
  assert.equal(
    baseline.design_baseline_commit,
    "f5863df1e6fcbde55a9b4b2ceeacd9e3c354d3c3",
  );
  assert.match(baseline.observed_commit, /^[0-9a-f]{40}$/);
  assert.equal(baseline.source_branch, "cabbos/forge-feishu-upgrade-sync-hook");
  assert.equal(baseline.base_branch, "main");
  assert.deepEqual(baseline.divergence, { behind: 0, ahead: 82 });
  assert.equal(baseline.design_status, "pending_user_review");

  assert.ok(Array.isArray(baseline.lockfiles));
  assert.equal(baseline.lockfiles.length, 4);
  for (const lockfile of baseline.lockfiles) {
    assert.equal(typeof lockfile.path, "string");
    assert.match(lockfile.sha256, /^[0-9a-f]{64}$/);
  }

  assert.deepEqual(baseline.gate_counts, {
    eval_pytest: { passed: 189, total: 189 },
    desktop_mocked_playwright: { passed: 40, total: 40 },
    acceptance_ci_default: { passed: 40, total: 40 },
    frontend_architecture: { passed: 27, total: 27 },
    stream_event_protocol_variants: { handled: 45, total: 45 },
  });
  assert.equal(baseline.known_red_tests.length, 1);
  assert.equal(baseline.warning_counts.desktop_tailwind_unknown_at_rules, 66);
  assert.equal(baseline.warning_counts.desktop_browser_console_errors, 1);

  assert.deepEqual(
    baseline.blockers.map(({ id }) => id),
    [
      "desktop-project-command-execution",
      "eval-persisted-execution-identity",
      "public-beta-distribution",
    ],
  );
  assert.equal(baseline.plans.length, 4);
});
