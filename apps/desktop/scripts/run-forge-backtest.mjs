import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import {
  dirname,
  join,
  resolve,
  relative,
} from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const CASE_FILE_NAMES = new Set(["case.json", "task.json"]);
const DEFAULT_PROVIDER = "forge";
const DEFAULT_FORGE_MODEL = "local-forge";
const DEFAULT_MOCK_MODEL = "deterministic-agent-v1";

export function selectCaseFiles(runnerRoot, { suite = "forge-session", caseIds = [] } = {}) {
  const evalCasesRoot = join(runnerRoot, "eval_cases");
  const allFiles = walkCaseFiles(evalCasesRoot);
  const wantedIds = new Set(caseIds);
  const selected = allFiles.filter((filePath) => {
    const ids = readCaseIds(filePath);
    if (wantedIds.size > 0) {
      return ids.some((id) => wantedIds.has(id));
    }
    if (suite === "all") {
      return true;
    }
    if (suite === "smoke") {
      return ids.includes("small-edit-success");
    }
    if (suite === "portable") {
      return ids.every((id) => !id.startsWith("forge-session-"));
    }
    if (suite === "continuity") {
      return ids.some((id) => id.startsWith("continuity-pipeline-"));
    }
    return ids.some((id) => id.startsWith("forge-session-"));
  });

  if (selected.length === 0) {
    throw new Error(`No eval cases matched suite '${suite}'.`);
  }

  return selected;
}

export function createSuiteCaseFile(caseFiles, outputDir, budgetOverrides = {}) {
  mkdirSync(outputDir, { recursive: true });
  const tasks = [];
  for (const caseFile of caseFiles) {
    const payload = readJson(caseFile);
    for (const task of extractTaskPayloads(payload)) {
      const resolved = resolveFixturePath(task, dirname(caseFile));
      tasks.push(applyBudgetOverrides(resolved, budgetOverrides));
    }
  }

  const suitePath = join(outputDir, "case.json");
  writeFileSync(
    suitePath,
    `${JSON.stringify({ schema_version: 1, tasks }, null, 2)}\n`,
    "utf8",
  );
  return suitePath;
}

export function buildBacktestPlan({
  repoRoot,
  runnerRoot,
  suitePath,
  outputPath,
  provider,
  model,
  env = process.env,
}) {
  const planEnv = { ...env };
  if (provider === "forge" && !planEnv.FORGE_EVAL_FORGE_AGENT_COMMAND) {
    planEnv.FORGE_EVAL_FORGE_AGENT_COMMAND = [
      "cargo",
      "run",
      "--manifest-path",
      join(repoRoot, "src-tauri", "Cargo.toml"),
      "--bin",
      "forge_eval_agent",
      "--quiet",
    ].join(" ");
  }

  return {
    command: "uv",
    args: [
      "run",
      "python",
      "-m",
      "app.cli",
      "--cases",
      suitePath,
      "--provider",
      provider,
      "--model",
      model,
      "--output",
      outputPath,
    ],
    cwd: runnerRoot,
    env: planEnv,
  };
}

function walkCaseFiles(root) {
  if (!existsSync(root)) {
    return [];
  }

  const files = [];
  const visit = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && CASE_FILE_NAMES.has(entry.name)) {
        files.push(path);
      }
    }
  };

  visit(root);
  return files.sort();
}

function readCaseIds(caseFile) {
  return extractTaskPayloads(readJson(caseFile)).map((task) => String(task.id ?? ""));
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function extractTaskPayloads(payload) {
  if (Array.isArray(payload)) {
    return payload.map(extractSingleTask);
  }
  if (payload && typeof payload === "object" && Array.isArray(payload.tasks)) {
    return payload.tasks.map(extractSingleTask);
  }
  return [extractSingleTask(payload)];
}

function extractSingleTask(payload) {
  const task = payload?.task ?? payload;
  if (!task || typeof task !== "object" || Array.isArray(task)) {
    throw new Error("Eval case task must be a JSON object.");
  }
  return { ...task };
}

function resolveFixturePath(task, baseDir) {
  if (!task.fixture_path) {
    return task;
  }
  const fixturePath = String(task.fixture_path);
  return {
    ...task,
    fixture_path: resolve(baseDir, fixturePath),
  };
}

function applyBudgetOverrides(task, overrides) {
  const result = { ...task };
  if (overrides.maxDurationSeconds !== undefined) {
    result.max_duration_seconds = overrides.maxDurationSeconds;
  }
  if (overrides.maxModelRounds !== undefined) {
    result.max_model_rounds = overrides.maxModelRounds;
  }
  return result;
}

function parseArgs(argv) {
  const options = {
    suite: "forge-session",
    caseIds: [],
    provider: DEFAULT_PROVIDER,
    model: null,
    runnerRoot: null,
    casesPath: null,
    outputPath: null,
    dryRun: false,
    maxDurationSeconds: undefined,
    maxModelRounds: undefined,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const nextValue = () => {
      index += 1;
      if (index >= argv.length) {
        throw new Error(`${arg} requires a value.`);
      }
      return argv[index];
    };

    if (arg === "--suite") {
      options.suite = nextValue();
    } else if (arg === "--case") {
      options.caseIds.push(...nextValue().split(",").filter(Boolean));
    } else if (arg === "--provider") {
      options.provider = nextValue();
    } else if (arg === "--model") {
      options.model = nextValue();
    } else if (arg === "--runner") {
      options.runnerRoot = nextValue();
    } else if (arg === "--cases") {
      options.casesPath = nextValue();
    } else if (arg === "--output") {
      options.outputPath = nextValue();
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--timeout") {
      options.maxDurationSeconds = Number(nextValue());
    } else if (arg === "--max-model-rounds") {
      options.maxModelRounds = Number(nextValue());
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function defaultRunnerRoot(repoRoot) {
  return resolve(
    process.env.FORGE_EVAL_RUNNER_PATH ?? join(repoRoot, "..", "forge-eval-runner"),
  );
}

function defaultOutputPath(repoRoot, suite, provider) {
  const timestamp = new Date()
    .toISOString()
    .replaceAll(":", "-")
    .replace(/\.\d{3}Z$/, "Z");
  return join(repoRoot, "artifacts", "eval-runs", `${timestamp}-${suite}-${provider}.json`);
}

function assertRunnerRoot(runnerRoot) {
  const cliPath = join(runnerRoot, "app", "cli.py");
  if (!existsSync(cliPath)) {
    throw new Error(`Forge eval runner not found at ${runnerRoot}. Expected ${cliPath}.`);
  }
}

function runCli(argv) {
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const options = parseArgs(argv);
  const runnerRoot = resolve(options.runnerRoot ?? defaultRunnerRoot(repoRoot));
  assertRunnerRoot(runnerRoot);

  const outputPath = resolve(
    options.outputPath ?? defaultOutputPath(repoRoot, options.suite, options.provider),
  );
  mkdirSync(dirname(outputPath), { recursive: true });

  const tempDir = mkdtempSync(join(tmpdir(), "forge-backtest-suite-"));
  const caseFiles = options.casesPath
    ? []
    : selectCaseFiles(runnerRoot, {
        suite: options.suite,
        caseIds: options.caseIds,
      });
  const budgetOverrides = {};
  if (options.maxDurationSeconds !== undefined) {
    budgetOverrides.maxDurationSeconds = options.maxDurationSeconds;
  }
  if (options.maxModelRounds !== undefined) {
    budgetOverrides.maxModelRounds = options.maxModelRounds;
  }
  const suitePath = options.casesPath
    ? resolve(options.casesPath)
    : createSuiteCaseFile(caseFiles, tempDir, budgetOverrides);
  const model =
    options.model ??
    (options.provider === "forge" ? DEFAULT_FORGE_MODEL : DEFAULT_MOCK_MODEL);
  const plan = buildBacktestPlan({
    repoRoot,
    runnerRoot,
    suitePath,
    outputPath,
    provider: options.provider,
    model,
    env: process.env,
  });
  const selectedCases = caseFiles.flatMap(readCaseIds);

  if (options.dryRun) {
    const suiteTasks = options.casesPath
      ? []
      : readJson(suitePath).tasks ?? [];
    const budgetSummary = suiteTasks.map((task) => ({
      id: task.id,
      timeout_secs: task.max_duration_seconds ?? null,
      max_model_rounds: task.max_model_rounds ?? null,
    }));
    console.log(
      JSON.stringify(
        {
          runnerRoot,
          suitePath,
          outputPath,
          selectedCases,
          budgetOverrides: Object.keys(budgetOverrides).length > 0 ? budgetOverrides : undefined,
          budgetSummary,
          command: [plan.command, ...plan.args],
          cwd: plan.cwd,
          env: {
            FORGE_EVAL_FORGE_AGENT_COMMAND:
              plan.env.FORGE_EVAL_FORGE_AGENT_COMMAND ?? null,
          },
        },
        null,
        2,
      ),
    );
    return 0;
  }

  console.log(`[forge-backtest] runner: ${runnerRoot}`);
  console.log(
    `[forge-backtest] cases: ${
      selectedCases.length > 0 ? selectedCases.join(", ") : relative(runnerRoot, suitePath)
    }`,
  );
  console.log(`[forge-backtest] output: ${outputPath}`);

  const result = spawnSync(plan.command, plan.args, {
    cwd: plan.cwd,
    env: plan.env,
    stdio: "inherit",
  });

  return result.status ?? 1;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.exitCode = runCli(process.argv.slice(2));
  } catch (error) {
    console.error(`[forge-backtest] ${error.message}`);
    process.exitCode = 1;
  }
}
