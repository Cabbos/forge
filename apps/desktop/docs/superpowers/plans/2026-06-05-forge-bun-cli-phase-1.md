# Forge Bun CLI Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Bun + TypeScript CLI skeleton with `forge doctor` and `forge run --json` wired to the existing Forge headless boundary through a mockable spawn layer.

**Architecture:** Create an isolated `cli/` package so the desktop frontend and Rust engine stay untouched. The CLI owns command parsing, path resolution, output formatting, and process spawning; Forge headless remains the execution source of truth through `cargo run --manifest-path src-tauri/Cargo.toml --bin forge_eval_agent --quiet`.

**Tech Stack:** Bun, TypeScript, `bun:test`, Node-compatible `fs/path/process` APIs, existing Rust `forge_eval_agent` binary.

---

## Scope

This plan implements the first development milestone only:

- `cli/` package skeleton
- `forge doctor`
- `forge run` request building and JSON rendering
- mocked spawn tests
- one optional real headless smoke command

This plan does not implement `forge eval` or `forge trace`. Those should be separate follow-up plans after `forge run --json` proves the CLI-to-headless contract.

## File Structure

- Create: `cli/package.json` - Bun package scripts and binary metadata.
- Create: `cli/tsconfig.json` - CLI TypeScript config.
- Create: `cli/src/index.ts` - executable entrypoint.
- Create: `cli/src/cli.ts` - command router and process exit handling.
- Create: `cli/src/commands/doctor.ts` - local environment readiness checks.
- Create: `cli/src/commands/run.ts` - prompt parsing, request building, headless invocation.
- Create: `cli/src/lib/headless.ts` - Forge headless command construction and JSON parsing.
- Create: `cli/src/lib/output.ts` - text/JSON render helpers.
- Create: `cli/src/lib/paths.ts` - repo root, cwd, and sibling path helpers.
- Create: `cli/src/lib/spawn.ts` - spawn abstraction used by commands and tests.
- Create: `cli/test/cli.test.ts` - router tests.
- Create: `cli/test/doctor.test.ts` - readiness check tests.
- Create: `cli/test/run.test.ts` - request building and mocked headless tests.
- Modify: `package.json` - add root scripts that delegate to the CLI package.

## Task 1: Create CLI Package Skeleton

**Files:**
- Create: `cli/package.json`
- Create: `cli/tsconfig.json`
- Create: `cli/src/index.ts`
- Create: `cli/src/cli.ts`
- Create: `cli/test/cli.test.ts`

- [ ] **Step 1: Create the package metadata**

Create `cli/package.json`:

```json
{
  "name": "@forge/cli",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "bin": {
    "forge": "./src/index.ts"
  },
  "scripts": {
    "dev": "bun run src/index.ts",
    "test": "bun test",
    "typecheck": "tsc --noEmit"
  },
  "devDependencies": {
    "@types/bun": "^1.2.0",
    "typescript": "^5.5.0"
  }
}
```

- [ ] **Step 2: Create TypeScript config**

Create `cli/tsconfig.json`:

```json
{
  "compilerOptions": {
    "lib": ["ESNext"],
    "target": "ESNext",
    "module": "Preserve",
    "moduleDetection": "force",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "noEmit": true,
    "strict": true,
    "skipLibCheck": true,
    "types": ["bun"],
    "noUncheckedIndexedAccess": true
  },
  "include": ["src/**/*.ts", "test/**/*.ts"]
}
```

- [ ] **Step 3: Write a failing router test**

Create `cli/test/cli.test.ts`:

```ts
import { describe, expect, test } from "bun:test";
import { runCli } from "../src/cli.ts";

function createIo() {
  const stdout: string[] = [];
  const stderr: string[] = [];
  return {
    stdout,
    stderr,
    io: {
      stdout: (text: string) => stdout.push(text),
      stderr: (text: string) => stderr.push(text),
    },
  };
}

describe("runCli", () => {
  test("prints help when no command is provided", async () => {
    const { io, stdout } = createIo();

    const code = await runCli([], { io });

    expect(code).toBe(0);
    expect(stdout.join("")).toContain("Usage: forge <command>");
    expect(stdout.join("")).toContain("doctor");
    expect(stdout.join("")).toContain("run");
  });

  test("returns non-zero for unknown commands", async () => {
    const { io, stderr } = createIo();

    const code = await runCli(["unknown"], { io });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Unknown command: unknown");
  });
});
```

- [ ] **Step 4: Run the failing test**

Run:

```bash
cd cli && bun test test/cli.test.ts
```

Expected:

```text
error: Cannot find module '../src/cli.ts'
```

- [ ] **Step 5: Implement CLI router**

Create `cli/src/cli.ts`:

```ts
import { runDoctor } from "./commands/doctor.ts";
import { runCommand } from "./commands/run.ts";
import type { SpawnRunner } from "./lib/spawn.ts";

export type CliIo = {
  stdout: (text: string) => void;
  stderr: (text: string) => void;
};

export type CliDeps = {
  io?: CliIo;
  spawn?: SpawnRunner;
  cwd?: string;
  env?: Record<string, string | undefined>;
};

const defaultIo: CliIo = {
  stdout: (text) => process.stdout.write(text),
  stderr: (text) => process.stderr.write(text),
};

export async function runCli(argv: string[], deps: CliDeps = {}): Promise<number> {
  const io = deps.io ?? defaultIo;
  const [command, ...rest] = argv;

  if (!command || command === "--help" || command === "-h") {
    io.stdout(helpText());
    return 0;
  }

  if (command === "doctor") {
    return runDoctor(rest, deps);
  }

  if (command === "run") {
    return runCommand(rest, deps);
  }

  io.stderr(`Unknown command: ${command}\n\n${helpText()}`);
  return 1;
}

export function helpText(): string {
  return [
    "Usage: forge <command> [options]",
    "",
    "Commands:",
    "  doctor        Check local Forge CLI readiness",
    "  run           Run one prompt through Forge headless",
    "",
  ].join("\n");
}
```

Create `cli/src/index.ts`:

```ts
#!/usr/bin/env bun

import { runCli } from "./cli.ts";

const exitCode = await runCli(Bun.argv.slice(2));
process.exit(exitCode);
```

Create temporary command stubs so Task 1 compiles:

```bash
mkdir -p cli/src/commands cli/src/lib
```

Create `cli/src/commands/doctor.ts`:

```ts
import type { CliDeps } from "../cli.ts";

export async function runDoctor(_argv: string[], deps: CliDeps = {}): Promise<number> {
  deps.io?.stdout("Forge doctor is not wired yet.\n");
  return 0;
}
```

Create `cli/src/commands/run.ts`:

```ts
import type { CliDeps } from "../cli.ts";

export async function runCommand(_argv: string[], deps: CliDeps = {}): Promise<number> {
  deps.io?.stdout("Forge run is not wired yet.\n");
  return 0;
}
```

Create `cli/src/lib/spawn.ts`:

```ts
export type SpawnInput = {
  command: string;
  args: string[];
  cwd: string;
  env?: Record<string, string | undefined>;
  stdin?: string;
};

export type SpawnOutput = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

export type SpawnRunner = (input: SpawnInput) => Promise<SpawnOutput>;

export const bunSpawnRunner: SpawnRunner = async (input) => {
  const proc = Bun.spawn([input.command, ...input.args], {
    cwd: input.cwd,
    env: compactEnv(input.env),
    stdin: input.stdin ? "pipe" : undefined,
    stdout: "pipe",
    stderr: "pipe",
  });

  if (input.stdin && proc.stdin) {
    proc.stdin.write(input.stdin);
    proc.stdin.end();
  }

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  return { exitCode, stdout, stderr };
};

function compactEnv(env: Record<string, string | undefined> | undefined) {
  if (!env) {
    return undefined;
  }
  return Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => typeof entry[1] === "string"),
  );
}
```

- [ ] **Step 6: Run router test**

Run:

```bash
cd cli && bun test test/cli.test.ts
```

Expected:

```text
2 pass
```

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add cli/package.json cli/tsconfig.json cli/src/index.ts cli/src/cli.ts cli/src/commands/doctor.ts cli/src/commands/run.ts cli/src/lib/spawn.ts cli/test/cli.test.ts
git commit -m "feat: add forge cli skeleton"
```

## Task 2: Add Path Helpers and Doctor Command

**Files:**
- Create: `cli/src/lib/paths.ts`
- Modify: `cli/src/commands/doctor.ts`
- Create: `cli/test/doctor.test.ts`

- [ ] **Step 1: Write failing doctor tests**

Create `cli/test/doctor.test.ts`:

```ts
import { describe, expect, test } from "bun:test";
import { runDoctor } from "../src/commands/doctor.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

function createIo() {
  const stdout: string[] = [];
  const stderr: string[] = [];
  return {
    stdout,
    stderr,
    io: {
      stdout: (text: string) => stdout.push(text),
      stderr: (text: string) => stderr.push(text),
    },
  };
}

describe("runDoctor", () => {
  test("reports readiness checks as JSON", async () => {
    const { io, stdout } = createIo();
    const spawn: SpawnRunner = async ({ command }) => ({
      exitCode: command === "bun" || command === "cargo" ? 0 : 1,
      stdout: command === "bun" ? "1.2.0\n" : "",
      stderr: "",
    });

    const code = await runDoctor(["--json"], {
      io,
      spawn,
      cwd: "/repo/forge",
      env: {
        FORGE_REPO_ROOT: "/repo/forge",
        FORGE_EVAL_RUNNER_ROOT: "/repo/forge-eval-runner",
      },
    });

    expect(code).toBe(0);
    const payload = JSON.parse(stdout.join(""));
    expect(payload.ok).toBe(true);
    expect(payload.checks.map((check: { name: string }) => check.name)).toEqual([
      "bun",
      "cargo",
      "forge_repo_root",
      "forge_eval_runner",
    ]);
  });

  test("human output marks failed checks", async () => {
    const { io, stdout } = createIo();
    const spawn: SpawnRunner = async () => ({ exitCode: 1, stdout: "", stderr: "missing" });

    const code = await runDoctor([], {
      io,
      spawn,
      cwd: "/repo/forge",
      env: {},
    });

    expect(code).toBe(1);
    expect(stdout.join("")).toContain("Forge doctor");
    expect(stdout.join("")).toContain("FAIL");
  });
});
```

- [ ] **Step 2: Run failing doctor tests**

Run:

```bash
cd cli && bun test test/doctor.test.ts
```

Expected:

```text
Expected: ["bun", "cargo", "forge_repo_root", "forge_eval_runner"]
Received: undefined
```

- [ ] **Step 3: Implement path helpers**

Create `cli/src/lib/paths.ts`:

```ts
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type PathEnv = Record<string, string | undefined>;

export function cliPackageRoot(): string {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
}

export function defaultForgeRepoRoot(env: PathEnv = process.env): string {
  if (env.FORGE_REPO_ROOT) {
    return resolve(env.FORGE_REPO_ROOT);
  }
  return resolve(cliPackageRoot(), "..");
}

export function defaultEvalRunnerRoot(forgeRepoRoot: string, env: PathEnv = process.env): string {
  if (env.FORGE_EVAL_RUNNER_ROOT) {
    return resolve(env.FORGE_EVAL_RUNNER_ROOT);
  }
  return resolve(forgeRepoRoot, "..", "forge-eval-runner");
}

export function isForgeRepoRoot(path: string): boolean {
  return existsSync(join(path, "src-tauri", "Cargo.toml")) && existsSync(join(path, "package.json"));
}

export function hasEvalRunner(path: string): boolean {
  return existsSync(join(path, "app")) && existsSync(join(path, "eval_cases"));
}
```

- [ ] **Step 4: Implement doctor command**

Replace `cli/src/commands/doctor.ts`:

```ts
import type { CliDeps } from "../cli.ts";
import { defaultEvalRunnerRoot, defaultForgeRepoRoot, hasEvalRunner, isForgeRepoRoot } from "../lib/paths.ts";
import { bunSpawnRunner, type SpawnRunner } from "../lib/spawn.ts";

type DoctorCheck = {
  name: string;
  ok: boolean;
  detail: string;
};

export async function runDoctor(argv: string[], deps: CliDeps = {}): Promise<number> {
  const io = deps.io ?? {
    stdout: (text: string) => process.stdout.write(text),
    stderr: (text: string) => process.stderr.write(text),
  };
  const json = argv.includes("--json");
  const spawn = deps.spawn ?? bunSpawnRunner;
  const env = deps.env ?? process.env;
  const forgeRepoRoot = defaultForgeRepoRoot(env);
  const evalRunnerRoot = defaultEvalRunnerRoot(forgeRepoRoot, env);

  const checks: DoctorCheck[] = [
    await commandCheck("bun", ["--version"], deps.cwd ?? forgeRepoRoot, spawn),
    await commandCheck("cargo", ["--version"], deps.cwd ?? forgeRepoRoot, spawn),
    {
      name: "forge_repo_root",
      ok: isForgeRepoRoot(forgeRepoRoot),
      detail: forgeRepoRoot,
    },
    {
      name: "forge_eval_runner",
      ok: hasEvalRunner(evalRunnerRoot),
      detail: evalRunnerRoot,
    },
  ];

  const ok = checks.every((check) => check.ok);

  if (json) {
    io.stdout(`${JSON.stringify({ ok, checks }, null, 2)}\n`);
  } else {
    io.stdout(renderDoctor(checks));
  }

  return ok ? 0 : 1;
}

async function commandCheck(
  command: string,
  args: string[],
  cwd: string,
  spawn: SpawnRunner,
): Promise<DoctorCheck> {
  const result = await spawn({ command, args, cwd });
  return {
    name: command,
    ok: result.exitCode === 0,
    detail: result.exitCode === 0 ? result.stdout.trim() : result.stderr.trim() || "not available",
  };
}

function renderDoctor(checks: DoctorCheck[]): string {
  const lines = ["Forge doctor", ""];
  for (const check of checks) {
    lines.push(`${check.ok ? "PASS" : "FAIL"} ${check.name}: ${check.detail}`);
  }
  lines.push("");
  return lines.join("\n");
}
```

- [ ] **Step 5: Run doctor tests**

Run:

```bash
cd cli && bun test test/doctor.test.ts
```

Expected:

```text
2 pass
```

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add cli/src/lib/paths.ts cli/src/commands/doctor.ts cli/test/doctor.test.ts
git commit -m "feat: add forge cli doctor"
```

## Task 3: Add Headless Request and Spawn Layer

**Files:**
- Create: `cli/src/lib/headless.ts`
- Create: `cli/src/lib/output.ts`
- Create: `cli/test/run.test.ts`

- [ ] **Step 1: Write failing headless tests**

Create `cli/test/run.test.ts`:

```ts
import { describe, expect, test } from "bun:test";
import { buildForgeHeadlessCommand, buildHeadlessRequest, runHeadlessJson } from "../src/lib/headless.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

describe("headless helpers", () => {
  test("builds the default cargo command", () => {
    const plan = buildForgeHeadlessCommand("/repo/forge");

    expect(plan.command).toBe("cargo");
    expect(plan.args).toEqual([
      "run",
      "--manifest-path",
      "/repo/forge/src-tauri/Cargo.toml",
      "--bin",
      "forge_eval_agent",
      "--quiet",
    ]);
  });

  test("builds a headless request", () => {
    expect(
      buildHeadlessRequest({
        prompt: "Fix tests",
        provider: "forge",
        model: "local-forge",
        workspacePath: "/work/app",
      }),
    ).toEqual({
      prompt: "Fix tests",
      provider: "forge",
      model: "local-forge",
      workspace_path: "/work/app",
    });
  });

  test("parses JSON from the headless process", async () => {
    const spawn: SpawnRunner = async (input) => {
      expect(input.stdin).toContain("\"prompt\":\"Fix tests\"");
      return {
        exitCode: 0,
        stdout: "{\"final_answer\":\"done\",\"changed_files\":[\"src/app.ts\"]}\\n",
        stderr: "",
      };
    };

    const result = await runHeadlessJson({
      forgeRepoRoot: "/repo/forge",
      request: buildHeadlessRequest({
        prompt: "Fix tests",
        provider: "forge",
        model: "local-forge",
        workspacePath: "/work/app",
      }),
      spawn,
    });

    expect(result.final_answer).toBe("done");
    expect(result.changed_files).toEqual(["src/app.ts"]);
  });
});
```

- [ ] **Step 2: Run failing headless tests**

Run:

```bash
cd cli && bun test test/run.test.ts
```

Expected:

```text
error: Cannot find module '../src/lib/headless.ts'
```

- [ ] **Step 3: Implement headless helpers**

Create `cli/src/lib/headless.ts`:

```ts
import { join } from "node:path";
import { bunSpawnRunner, type SpawnRunner } from "./spawn.ts";

export type HeadlessRequest = {
  prompt: string;
  provider: string;
  model: string;
  workspace_path: string;
};

export type BuildHeadlessRequestInput = {
  prompt: string;
  provider: string;
  model: string;
  workspacePath: string;
};

export type HeadlessCommandPlan = {
  command: string;
  args: string[];
};

export function buildHeadlessRequest(input: BuildHeadlessRequestInput): HeadlessRequest {
  return {
    prompt: input.prompt,
    provider: input.provider,
    model: input.model,
    workspace_path: input.workspacePath,
  };
}

export function buildForgeHeadlessCommand(forgeRepoRoot: string): HeadlessCommandPlan {
  return {
    command: "cargo",
    args: [
      "run",
      "--manifest-path",
      join(forgeRepoRoot, "src-tauri", "Cargo.toml"),
      "--bin",
      "forge_eval_agent",
      "--quiet",
    ],
  };
}

export async function runHeadlessJson(input: {
  forgeRepoRoot: string;
  request: HeadlessRequest;
  spawn?: SpawnRunner;
}): Promise<Record<string, unknown>> {
  const spawn = input.spawn ?? bunSpawnRunner;
  const plan = buildForgeHeadlessCommand(input.forgeRepoRoot);
  const result = await spawn({
    command: plan.command,
    args: plan.args,
    cwd: input.forgeRepoRoot,
    stdin: `${JSON.stringify(input.request)}\n`,
  });

  if (result.exitCode !== 0) {
    throw new Error(result.stderr.trim() || `Forge headless exited with code ${result.exitCode}`);
  }

  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`Forge headless returned invalid JSON: ${(error as Error).message}`);
  }
}
```

Create `cli/src/lib/output.ts`:

```ts
export function renderJson(payload: unknown): string {
  return `${JSON.stringify(payload, null, 2)}\n`;
}

export function renderRunSummary(payload: Record<string, unknown>): string {
  const changedFiles = Array.isArray(payload.changed_files) ? payload.changed_files.length : 0;
  const validation = validationStatus(payload);
  const finalAnswer = typeof payload.final_answer === "string" ? payload.final_answer : "";

  return [
    "Forge run completed",
    "",
    `Provider: ${String(payload.provider ?? "forge")}`,
    `Model: ${String(payload.model ?? "local-forge")}`,
    `Changed files: ${changedFiles}`,
    `Validation: ${validation}`,
    "",
    "Final answer:",
    finalAnswer,
    "",
  ].join("\n");
}

function validationStatus(payload: Record<string, unknown>): string {
  if (typeof payload.validation_status === "string") {
    return payload.validation_status;
  }
  if (typeof payload.failure_category === "string") {
    return "failed";
  }
  return "unknown";
}
```

- [ ] **Step 4: Run headless helper tests**

Run:

```bash
cd cli && bun test test/run.test.ts
```

Expected:

```text
3 pass
```

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add cli/src/lib/headless.ts cli/src/lib/output.ts cli/test/run.test.ts
git commit -m "feat: add forge cli headless helpers"
```

## Task 4: Implement `forge run`

**Files:**
- Modify: `cli/src/commands/run.ts`
- Modify: `cli/test/run.test.ts`

- [ ] **Step 1: Add failing command tests**

Append to `cli/test/run.test.ts`:

```ts
import { runCommand } from "../src/commands/run.ts";

function createIo() {
  const stdout: string[] = [];
  const stderr: string[] = [];
  return {
    stdout,
    stderr,
    io: {
      stdout: (text: string) => stdout.push(text),
      stderr: (text: string) => stderr.push(text),
    },
  };
}

describe("runCommand", () => {
  test("requires a prompt", async () => {
    const { io, stderr } = createIo();

    const code = await runCommand([], { io, cwd: "/work/app" });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge run");
  });

  test("runs prompt and prints JSON", async () => {
    const { io, stdout } = createIo();

    const code = await runCommand(["--json", "--cwd", "/work/app", "Fix tests"], {
      io,
      cwd: "/repo/forge",
      env: { FORGE_REPO_ROOT: "/repo/forge" },
      spawn: async (input) => {
        expect(input.cwd).toBe("/repo/forge");
        expect(input.stdin).toContain("\"workspace_path\":\"/work/app\"");
        return {
          exitCode: 0,
          stdout: "{\"provider\":\"forge\",\"model\":\"local-forge\",\"final_answer\":\"done\",\"changed_files\":[]}",
          stderr: "",
        };
      },
    });

    expect(code).toBe(0);
    expect(JSON.parse(stdout.join("")).final_answer).toBe("done");
  });
});
```

- [ ] **Step 2: Run failing command tests**

Run:

```bash
cd cli && bun test test/run.test.ts
```

Expected:

```text
Expected stdout JSON with final_answer "done"
```

- [ ] **Step 3: Implement `forge run`**

Replace `cli/src/commands/run.ts`:

```ts
import { resolve } from "node:path";
import type { CliDeps } from "../cli.ts";
import { buildHeadlessRequest, runHeadlessJson } from "../lib/headless.ts";
import { renderJson, renderRunSummary } from "../lib/output.ts";
import { defaultForgeRepoRoot } from "../lib/paths.ts";

type RunOptions = {
  json: boolean;
  cwd: string;
  provider: string;
  model: string;
  prompt: string;
};

export async function runCommand(argv: string[], deps: CliDeps = {}): Promise<number> {
  const io = deps.io ?? {
    stdout: (text: string) => process.stdout.write(text),
    stderr: (text: string) => process.stderr.write(text),
  };

  const parsed = parseRunArgs(argv, deps.cwd ?? process.cwd());
  if (!parsed.ok) {
    io.stderr(`${parsed.error}\n\n${runHelpText()}`);
    return 1;
  }

  const forgeRepoRoot = defaultForgeRepoRoot(deps.env ?? process.env);
  const request = buildHeadlessRequest({
    prompt: parsed.options.prompt,
    provider: parsed.options.provider,
    model: parsed.options.model,
    workspacePath: parsed.options.cwd,
  });

  try {
    const payload = await runHeadlessJson({
      forgeRepoRoot,
      request,
      spawn: deps.spawn,
    });
    io.stdout(parsed.options.json ? renderJson(payload) : renderRunSummary(payload));
    return 0;
  } catch (error) {
    io.stderr(`${(error as Error).message}\n`);
    return 1;
  }
}

function parseRunArgs(argv: string[], defaultCwd: string):
  | { ok: true; options: RunOptions }
  | { ok: false; error: string } {
  const options: Omit<RunOptions, "prompt"> = {
    json: false,
    cwd: resolve(defaultCwd),
    provider: "forge",
    model: "local-forge",
  };
  const promptParts: string[] = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--cwd") {
      const value = argv[index + 1];
      if (!value) {
        return { ok: false, error: "Missing value for --cwd" };
      }
      options.cwd = resolve(value);
      index += 1;
    } else if (arg === "--provider") {
      const value = argv[index + 1];
      if (!value) {
        return { ok: false, error: "Missing value for --provider" };
      }
      options.provider = value;
      index += 1;
    } else if (arg === "--model") {
      const value = argv[index + 1];
      if (!value) {
        return { ok: false, error: "Missing value for --model" };
      }
      options.model = value;
      index += 1;
    } else {
      promptParts.push(arg);
    }
  }

  const prompt = promptParts.join(" ").trim();
  if (!prompt) {
    return { ok: false, error: "Missing prompt" };
  }

  return { ok: true, options: { ...options, prompt } };
}

function runHelpText(): string {
  return [
    "Usage: forge run [options] <prompt>",
    "",
    "Options:",
    "  --cwd <path>        Workspace path for the task",
    "  --provider <name>   Provider label passed to Forge headless",
    "  --model <name>      Model label passed to Forge headless",
    "  --json              Print raw JSON result",
    "",
  ].join("\n");
}
```

- [ ] **Step 4: Run command tests**

Run:

```bash
cd cli && bun test test/run.test.ts
```

Expected:

```text
5 pass
```

- [ ] **Step 5: Run full CLI test suite**

Run:

```bash
cd cli && bun test
```

Expected:

```text
all tests pass
```

- [ ] **Step 6: Commit Task 4**

Run:

```bash
git add cli/src/commands/run.ts cli/test/run.test.ts
git commit -m "feat: add forge cli run command"
```

## Task 5: Add Root Scripts and Verification

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Run GitNexus impact check for package script change**

Run:

```bash
npx gitnexus analyze
```

Expected:

```text
Repository indexed successfully
```

No symbol impact analysis is required for new CLI files. If this task expands into existing function edits, run `gitnexus_impact` on each existing function before editing it.

- [ ] **Step 2: Add root scripts**

Modify the root `package.json` scripts block by adding these entries:

```json
{
  "cli": "bun --cwd cli run src/index.ts",
  "cli:test": "bun --cwd cli test",
  "cli:typecheck": "bun --cwd cli run typecheck",
  "cli:doctor": "bun --cwd cli run src/index.ts doctor"
}
```

Keep all existing scripts unchanged.

- [ ] **Step 3: Run CLI tests from root**

Run:

```bash
npm run cli:test
```

Expected:

```text
all tests pass
```

- [ ] **Step 4: Run CLI typecheck from root**

Run:

```bash
npm run cli:typecheck
```

Expected:

```text
no TypeScript errors
```

- [ ] **Step 5: Run doctor command**

Run:

```bash
npm run cli:doctor
```

Expected:

```text
Forge doctor
PASS bun: ...
PASS cargo: ...
PASS forge_repo_root: /Users/cabbos/project/crusted-spinning-lynx-agent
PASS forge_eval_runner: /Users/cabbos/project/forge-eval-runner
```

If `forge_eval_runner` fails because the sibling repo is absent, keep the failure and document it in the final result. Do not change the fallback path to hide the missing repo.

- [ ] **Step 6: Run existing repo-safe checks**

Run:

```bash
npm run check:precommit:test
```

Expected:

```text
tests pass
```

- [ ] **Step 7: Detect staged changes before commit**

Run after staging only CLI Phase 1 files:

```bash
git add cli package.json
npx gitnexus analyze
```

Then run GitNexus staged change detection:

```text
gitnexus_detect_changes(scope: "staged", repo: "forge-v1")
```

Expected:

```text
risk_level: low or medium
changed files limited to cli/** and package.json
```

- [ ] **Step 8: Commit Task 5**

Run:

```bash
git commit -m "chore: wire forge cli scripts"
```

## Optional Smoke Check

Run this only after the mocked test suite passes:

```bash
npm run cli -- run --json --cwd /Users/cabbos/project/forge-test-app "Summarize the current project in one sentence."
```

Expected:

```text
valid JSON from Forge headless
```

If this fails with `missing_api_key`, the CLI path is still wired correctly; report the missing API key as environment readiness, not as a CLI implementation failure.

## Follow-Up Plans

After Phase 1 is committed and verified:

- Phase 2 plan: migrate `scripts/run-forge-backtest.mjs` behavior into `forge eval`.
- Phase 3 plan: add artifact discovery and Markdown/JSON export in `forge trace`.
- Phase 4 plan: compile the CLI with Bun once command behavior is stable.
