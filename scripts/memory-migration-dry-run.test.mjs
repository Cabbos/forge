import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "memory-migration-dry-run.mjs");

function runJson(args = []) {
  return JSON.parse(
    execFileSync(process.execPath, [scriptPath, "--json", ...args], {
      cwd: root,
      encoding: "utf8",
    }),
  );
}

test("emits a read-only physical migration design report", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/memory-migration-dry-run.mjs should exist");

  const report = runJson();

  assert.equal(report.schemaVersion, 1);
  assert.equal(report.mode, "dry-run");
  assert.equal(report.writesPerformed, false);
  assert.equal(report.physicalStoreMigrationStarted, false);
  assert.equal(report.readyForPhysicalMigration, false);
  assert.equal(report.targetStore, "unified_sqlite");
  assert.ok(report.sourceStores.some((store) => store.source === "wiki_memory"));
  assert.ok(report.sourceStores.some((store) => store.source === "memory_fact"));
  assert.ok(report.sourceStores.some((store) => store.source === "continuity_experience"));
  assert.ok(report.records.length >= 3, "default fixture should cover the existing memory sources");
  assert.ok(
    report.invariants.every((invariant) => invariant.passed),
    "built-in representative records should satisfy all dry-run invariants",
  );
  assert.ok(report.rollbackPlan.length >= 5);
  assert.ok(report.summary.blockers.includes("operator_approval_required"));
});

test("fixture report preserves ids, action semantics, recall decisions, and hidden bodies", () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-memory-migration-"));
  try {
    const fixturePath = join(dir, "fixture.json");
    writeFileSync(
      fixturePath,
      JSON.stringify(
        {
          records: [
            {
              id: "wiki_memory:w-1",
              source: "wiki_memory",
              source_id: "w-1",
              status: "accepted",
              visibility: "user_visible",
              title: "Project decision",
              body: "The body must not appear in the dry-run report.",
            },
            {
              id: "memory_fact:f-1",
              source: "memory_fact",
              source_id: "f-1",
              status: "accepted",
              visibility: "hidden_context",
              text: "Hidden fact body must not leak.",
            },
            {
              id: "continuity_experience:c-1",
              source: "continuity_experience",
              source_id: "c-1",
              status: "archived",
              visibility: "audit_only",
              content: "Archived body must not leak.",
            },
          ],
          expectedRecallIds: ["wiki_memory:w-1", "memory_fact:f-1"],
        },
        null,
        2,
      ),
    );

    const report = runJson(["--fixture", fixturePath]);

    assert.deepEqual(
      report.records.map((record) => record.id),
      ["wiki_memory:w-1", "memory_fact:f-1", "continuity_experience:c-1"],
    );
    assert.deepEqual(report.summary.recordCounts, {
      total: 3,
      archived: 1,
      forgotten: 0,
      recallEligible: 2,
    });
    assert.equal(
      report.invariants.find((invariant) => invariant.id === "record_identity_stable").passed,
      true,
    );
    assert.equal(
      report.invariants.find((invariant) => invariant.id === "archive_forget_semantics_stable")
        .passed,
      true,
    );
    assert.equal(
      report.invariants.find((invariant) => invariant.id === "recall_results_stable").passed,
      true,
    );
    assert.equal(
      report.invariants.find((invariant) => invariant.id === "hidden_bodies_not_exported").passed,
      true,
    );
    assert.ok(report.records.every((record) => !("body" in record)));
    assert.ok(report.records.every((record) => !("text" in record)));
    assert.ok(report.records.every((record) => !("content" in record)));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("--out writes the report without marking a physical migration write", () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-memory-migration-out-"));
  try {
    const outPath = join(dir, "report.json");

    const report = runJson(["--out", outPath]);
    const writtenReport = JSON.parse(readFileSync(outPath, "utf8"));

    assert.equal(report.writesPerformed, false);
    assert.equal(writtenReport.writesPerformed, false);
    assert.equal(writtenReport.reportWritePerformed, true);
    assert.equal(writtenReport.physicalStoreMigrationStarted, false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("rejects unknown arguments", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--json", "--unknown"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Unknown argument: --unknown/);
});
