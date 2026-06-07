import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import test from "node:test";

import {
  buildBacktestPlan,
  createSuiteCaseFile,
  selectCaseFiles,
} from "./run-forge-backtest.mjs";

function makeRunnerFixture() {
  const root = mkdtempSync(join(tmpdir(), "forge-backtest-runner-"));
  const cases = join(root, "eval_cases");
  const fixture = join(cases, "_fixtures", "app");
  const forgeCase = join(cases, "forge-session-example");
  const continuityCase = join(cases, "continuity-pipeline-example");
  const portableCase = join(cases, "small-edit-success");

  mkdirSync(fixture, { recursive: true });
  mkdirSync(forgeCase, { recursive: true });
  mkdirSync(continuityCase, { recursive: true });
  mkdirSync(portableCase, { recursive: true });
  writeFileSync(join(root, "pyproject.toml"), "[project]\nname = \"runner\"\n");
  writeFileSync(join(root, "README.md"), "runner\n");
  writeFileSync(join(fixture, ".keep"), "");

  return { root, cases, fixture, forgeCase, continuityCase, portableCase };
}

function writeCase(caseDir, id, fixturePath = "../_fixtures/app") {
  writeFileSync(
    join(caseDir, "case.json"),
    JSON.stringify(
      {
        schema_version: 1,
        task: {
          id,
          title: id,
          prompt: `Run ${id}`,
          fixture_path: fixturePath,
          validation_commands: ["npm test"],
          expected_success: true,
        },
      },
      null,
      2,
    ),
  );
}

test("selects only forge-session cases by default", () => {
  const fixture = makeRunnerFixture();
  try {
    writeCase(fixture.forgeCase, "forge-session-example");
    writeCase(fixture.portableCase, "small-edit-success");

    const selected = selectCaseFiles(fixture.root, {
      suite: "forge-session",
      caseIds: [],
    });

    assert.deepEqual(
      selected.map((file) => basename(dirname(file))),
      ["forge-session-example"],
    );
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("creates an aggregate case file with absolute fixture paths", () => {
  const fixture = makeRunnerFixture();
  const outputDir = mkdtempSync(join(tmpdir(), "forge-backtest-output-"));
  try {
    writeCase(fixture.forgeCase, "forge-session-example");

    const suiteFile = createSuiteCaseFile(
      selectCaseFiles(fixture.root, { suite: "forge-session", caseIds: [] }),
      outputDir,
    );
    const payload = JSON.parse(readFileSync(suiteFile, "utf8"));

    assert.equal(payload.tasks[0].id, "forge-session-example");
    assert.equal(payload.tasks[0].fixture_path, fixture.fixture);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
    rmSync(outputDir, { recursive: true, force: true });
  }
});

test("selects continuity pipeline cases by suite", () => {
  const fixture = makeRunnerFixture();
  try {
    writeCase(fixture.forgeCase, "forge-session-example");
    writeCase(fixture.continuityCase, "continuity-pipeline-example");
    writeCase(fixture.portableCase, "small-edit-success");

    const selected = selectCaseFiles(fixture.root, {
      suite: "continuity",
      caseIds: [],
    });

    assert.deepEqual(
      selected.map((file) => basename(dirname(file))),
      ["continuity-pipeline-example"],
    );
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("plans Forge provider command with default headless binary", () => {
  const repoRoot = "/repo/forge";
  const runnerRoot = "/repo/forge-eval-runner";
  const outputPath = "/repo/forge/artifacts/eval-runs/report.json";
  const suitePath = "/tmp/forge-suite.json";

  const plan = buildBacktestPlan({
    repoRoot,
    runnerRoot,
    suitePath,
    outputPath,
    provider: "forge",
    model: "local-forge",
    env: {},
  });

  assert.equal(plan.cwd, runnerRoot);
  assert.deepEqual(plan.args, [
    "run",
    "python",
    "-m",
    "app.cli",
    "--cases",
    suitePath,
    "--provider",
    "forge",
    "--model",
    "local-forge",
    "--output",
    outputPath,
  ]);
  assert.equal(
    plan.env.FORGE_EVAL_FORGE_AGENT_COMMAND,
    "cargo run --manifest-path /repo/forge/src-tauri/Cargo.toml --bin forge_eval_agent --quiet",
  );
});

test("createSuiteCaseFile applies budget overrides to all tasks", () => {
  const fixture = makeRunnerFixture();
  const outputDir = mkdtempSync(join(tmpdir(), "forge-backtest-budget-"));
  try {
    writeCase(fixture.forgeCase, "task-a");
    writeCase(fixture.portableCase, "task-b");

    const suiteFile = createSuiteCaseFile(
      selectCaseFiles(fixture.root, { suite: "all", caseIds: [] }),
      outputDir,
      { maxDurationSeconds: 120, maxModelRounds: 50 },
    );
    const payload = JSON.parse(readFileSync(suiteFile, "utf8"));

    assert.equal(payload.tasks.length, 2);
    for (const task of payload.tasks) {
      assert.equal(task.max_duration_seconds, 120);
      assert.equal(task.max_model_rounds, 50);
    }
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
    rmSync(outputDir, { recursive: true, force: true });
  }
});

test("createSuiteCaseFile preserves existing budget when no override", () => {
  const fixture = makeRunnerFixture();
  const outputDir = mkdtempSync(join(tmpdir(), "forge-backtest-budget-"));
  try {
    writeFileSync(
      join(fixture.forgeCase, "case.json"),
      JSON.stringify(
        {
          schema_version: 1,
          task: {
            id: "forge-session-budget-task",
            title: "budget-task",
            prompt: "Run budget-task",
            fixture_path: "../_fixtures/app",
            max_duration_seconds: 30,
            max_model_rounds: 10,
            expected_success: true,
          },
        },
        null,
        2,
      ),
    );

    const suiteFile = createSuiteCaseFile(
      selectCaseFiles(fixture.root, { suite: "forge-session", caseIds: [] }),
      outputDir,
    );
    const payload = JSON.parse(readFileSync(suiteFile, "utf8"));

    assert.equal(payload.tasks[0].max_duration_seconds, 30);
    assert.equal(payload.tasks[0].max_model_rounds, 10);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
    rmSync(outputDir, { recursive: true, force: true });
  }
});
