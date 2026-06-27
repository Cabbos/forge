import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { evaluateDisposableLoopProject } from "./disposable-loop-preflight.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "disposable-loop-preflight.mjs");

test("reports a clean disposable project as ready", (t) => {
  const projectPath = createDisposableProject(t);
  const result = evaluateDisposableLoopProject({ projectPath });

  assert.equal(result.status, "ready");
  assert.equal(result.readyForLoop, true);
  assert.equal(result.git.clean, true);
  assert.equal(result.git.rootMatchesProject, true);
  assert.equal(result.package.name, "forge-test-app");
  assert.equal(result.package.hasBuildScript, true);
  assert.deepEqual(result.requiredFiles.map(({ file, exists }) => [file, exists]), [
    ["src/App.tsx", true],
    ["src/styles.css", true],
  ]);
});

test("blocks fresh evidence when the disposable worktree is dirty", (t) => {
  const projectPath = createDisposableProject(t);
  appendFileSync(join(projectPath, "src", "styles.css"), "\n.button { opacity: 0.95; }\n");

  const result = evaluateDisposableLoopProject({ projectPath });

  assert.equal(result.status, "dirty_worktree");
  assert.equal(result.readyForLoop, false);
  assert.equal(result.git.clean, false);
  assert.deepEqual(result.git.dirtyFiles, ["M src/styles.css"]);
  assert.match(result.nextStep, /existing disposable-project changes/);
});

test("reports missing build script precisely", (t) => {
  const projectPath = createDisposableProject(t, {
    packageJson: {
      name: "forge-test-app",
      version: "0.1.0",
      private: true,
      scripts: {
        dev: "vite --host 127.0.0.1",
      },
    },
  });

  const result = evaluateDisposableLoopProject({ projectPath });

  assert.equal(result.status, "missing_build_script");
  assert.equal(result.readyForLoop, false);
  assert.equal(result.package.hasBuildScript, false);
});

test("json mode emits machine-readable missing-project status", () => {
  const missingPath = join(tmpdir(), `forge-missing-${Date.now()}`);
  const output = execFileSync(process.execPath, [scriptPath, "--json", "--project", missingPath], {
    cwd: root,
    encoding: "utf8",
  });
  const result = JSON.parse(output);

  assert.equal(result.status, "missing_project");
  assert.equal(result.readyForLoop, false);
  assert.equal(result.projectPath, missingPath);
});

test("require-ready exits nonzero when the project is not ready", () => {
  const missingPath = join(tmpdir(), `forge-missing-${Date.now()}`);

  assert.throws(
    () =>
      execFileSync(process.execPath, [scriptPath, "--json", "--require-ready", "--project", missingPath], {
        cwd: root,
        encoding: "utf8",
      }),
    (error) => error.status === 1,
  );
});

function createDisposableProject(t, { packageJson = defaultPackageJson() } = {}) {
  const projectPath = mkdtempSync(join(tmpdir(), "forge-disposable-loop-"));
  t.after(() => rmSync(projectPath, { recursive: true, force: true }));

  mkdirSync(join(projectPath, "src"), { recursive: true });
  writeFileSync(join(projectPath, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);
  writeFileSync(
    join(projectPath, "src", "App.tsx"),
    "export default function App() {\n  return <button>Demo</button>;\n}\n",
  );
  writeFileSync(join(projectPath, "src", "styles.css"), "button { transition: opacity 120ms ease; }\n");

  git(projectPath, ["init"]);
  git(projectPath, ["config", "user.email", "forge@example.local"]);
  git(projectPath, ["config", "user.name", "Forge Test"]);
  git(projectPath, ["add", "."]);
  git(projectPath, ["commit", "-m", "initial disposable project"]);

  return projectPath;
}

function defaultPackageJson() {
  return {
    name: "forge-test-app",
    version: "0.1.0",
    private: true,
    type: "module",
    scripts: {
      dev: "vite --host 127.0.0.1",
      build: "tsc && vite build",
    },
  };
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
