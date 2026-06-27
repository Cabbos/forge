import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { evaluateDisposableLoopProject } from "./disposable-loop-preflight.mjs";
import { prepareDisposableLoopProject } from "./prepare-disposable-loop-project.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "prepare-disposable-loop-project.mjs");

test("prepares a clean target from a dirty source without changing the source", (t) => {
  const { sourcePath, targetPath } = createDirtySourceProject(t);

  const result = prepareDisposableLoopProject({ sourcePath, targetPath });

  assert.equal(result.status, "prepared");
  assert.equal(result.prepared, true);
  assert.equal(result.source.gitClean, false);
  assert.deepEqual(result.source.dirtyFiles, ["M src/styles.css"]);
  assert.equal(result.target.readyForLoop, true);
  assert.equal(result.target.gitClean, true);
  assert.equal(result.nodeModules.linked, true);
  assert.equal(lstatSync(join(targetPath, "node_modules")).isSymbolicLink(), true);
  assert.equal(readFileSync(join(targetPath, "src", "styles.css"), "utf8"), "button { opacity: 1; }\n");
  assert.match(readFileSync(join(sourcePath, "src", "styles.css"), "utf8"), /transform: scale/);

  const targetPreflight = evaluateDisposableLoopProject({ projectPath: targetPath });
  assert.equal(targetPreflight.readyForLoop, true);
});

test("dry-run reports preparation commands without creating the target", (t) => {
  const { sourcePath, targetPath } = createDirtySourceProject(t);

  const result = prepareDisposableLoopProject({ sourcePath, targetPath, dryRun: true });

  assert.equal(result.status, "dry_run_ready");
  assert.equal(result.prepared, false);
  assert.equal(result.dryRun, true);
  assert.equal(existsSync(targetPath), false);
  assert.equal(result.commands.length, 2);
  assert.match(result.commands[0], /git -C .* worktree add --detach .* HEAD/);
});

test("existing ready target can be reused", (t) => {
  const { sourcePath, targetPath } = createDirtySourceProject(t);
  prepareDisposableLoopProject({ sourcePath, targetPath });

  const result = prepareDisposableLoopProject({ sourcePath, targetPath, dryRun: true });

  assert.equal(result.status, "target_ready");
  assert.equal(result.prepared, true);
  assert.equal(result.target.readyForLoop, true);
  assert.deepEqual(result.issues, []);
});

test("existing target that is not ready is refused", (t) => {
  const { sourcePath, targetPath } = createDirtySourceProject(t);
  mkdirSync(targetPath);

  const result = prepareDisposableLoopProject({ sourcePath, targetPath });

  assert.equal(result.status, "target_exists_not_ready");
  assert.equal(result.prepared, false);
  assert.equal(result.targetExists, true);
  assert.match(result.nextStep, /inspect and fix the existing target/);
});

test("cli dry-run emits machine-readable plan", (t) => {
  const { sourcePath, targetPath } = createDirtySourceProject(t);
  const output = execFileSync(
    process.execPath,
    [scriptPath, "--json", "--dry-run", "--source", sourcePath, "--target", targetPath],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const result = JSON.parse(output);

  assert.equal(result.status, "dry_run_ready");
  assert.equal(result.source.readyForLoop, false);
  assert.equal(result.source.dirtyFiles[0], "M src/styles.css");
  assert.equal(result.targetPath, targetPath);
  assert.equal(existsSync(targetPath), false);
});

function createDirtySourceProject(t) {
  const parentPath = mkdtempSync(join(tmpdir(), "forge-prepare-loop-"));
  t.after(() => rmSync(parentPath, { recursive: true, force: true }));

  const sourcePath = join(parentPath, "forge-test-app");
  const targetPath = join(parentPath, "forge-test-app-phase8-clean");
  mkdirSync(join(sourcePath, "src"), { recursive: true });
  mkdirSync(join(sourcePath, "node_modules"), { recursive: true });
  writeFileSync(join(sourcePath, ".gitignore"), "node_modules\n");
  writeFileSync(join(sourcePath, "node_modules", ".keep"), "");
  writeFileSync(
    join(sourcePath, "package.json"),
    `${JSON.stringify(
      {
        name: "forge-test-app",
        version: "0.1.0",
        private: true,
        type: "module",
        scripts: {
          dev: "vite --host 127.0.0.1",
          build: "tsc && vite build",
        },
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(join(sourcePath, "src", "App.tsx"), "export default function App() {\n  return <button>Demo</button>;\n}\n");
  writeFileSync(join(sourcePath, "src", "styles.css"), "button { opacity: 1; }\n");

  git(sourcePath, ["init"]);
  git(sourcePath, ["config", "user.email", "forge@example.local"]);
  git(sourcePath, ["config", "user.name", "Forge Test"]);
  git(sourcePath, ["add", "."]);
  git(sourcePath, ["commit", "-m", "initial disposable project"]);

  appendFileSync(join(sourcePath, "src", "styles.css"), ".button:active { transform: scale(0.97); }\n");

  return { sourcePath, targetPath };
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
