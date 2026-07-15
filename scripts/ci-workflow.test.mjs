import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  const absolutePath = join(root, path);
  assert.equal(existsSync(absolutePath), true, `${path} should exist`);
  return readFileSync(absolutePath, "utf8");
}

test("CI workflow covers the monorepo quality gates", () => {
  const workflow = read(".github/workflows/ci.yml");
  const rootPackage = JSON.parse(read("package.json"));

  assert.match(workflow, /pull_request:/);
  assert.match(workflow, /push:/);
  assert.match(workflow, /branches:\s*\[\s*main\s*\]/);
  assert.match(workflow, /paths:/);
  assert.match(workflow, /apps\/desktop\/\*\*/);
  assert.match(workflow, /apps\/eval-runner\/\*\*/);
  assert.match(workflow, /apps\/website\/\*\*/);

  assert.match(workflow, /desktop-frontend:/);
  assert.match(workflow, /npm run check:frontend-architecture/);
  assert.match(workflow, /npm run build/);

  assert.match(workflow, /desktop-backend:/);
  assert.match(workflow, /desktop-backend:[\s\S]*?runs-on:\s*macos-latest/);
  assert.match(workflow, /desktop-backend:[\s\S]*?npm run build/);
  assert.match(workflow, /cargo fmt[\s\S]*?--check/);
  assert.match(workflow, /cargo clippy[\s\S]*?--all-targets -- -D warnings/);
  assert.match(workflow, /cargo test[\s\S]*?src-tauri\/Cargo.toml/);

  assert.match(workflow, /eval-runner:/);
  assert.match(workflow, /uv sync --frozen --dev/);
  assert.match(workflow, /uv run pytest/);

  assert.match(workflow, /website:/);
  assert.match(workflow, /npm run build/);

  assert.match(workflow, /nightly-eval:/);
  assert.match(workflow, /schedule:/);
  assert.match(workflow, /upload-artifact/);

  // Real smoke must not run on PR/push (only nightly / workflow_dispatch)
  const nightlySection = workflow.slice(workflow.indexOf("nightly-eval:"));
  const prPushSection = workflow.slice(0, workflow.indexOf("nightly-eval:"));
  assert.doesNotMatch(prPushSection, /smoke:real/);
  assert.match(nightlySection, /smoke:real/);

  // Nightly should retain the exact mock and real Eval reports and require a real API key.
  assert.match(nightlySection, /mock-backtest\.json/);
  assert.match(nightlySection, /real-forge-backtest\.json/);
  assert.match(nightlySection, /ANTHROPIC_API_KEY/);
  assert.doesNotMatch(nightlySection, /Skipping real smoke/);
  assert.match(nightlySection, /ANTHROPIC_API_KEY is required for trusted real-Forge evidence/);

  const forwardedRootScripts = [
    "eval:forge:mock",
    "eval:forge:smoke",
    "eval:forge:smoke:real",
    "eval:forge:smoke:dry-run",
    "eval:continuity",
    "eval:report",
    "eval:report:latest",
  ];
  for (const scriptName of forwardedRootScripts) {
    const script = rootPackage.scripts[scriptName];
    assert.match(
      script,
      /npm --prefix apps\/desktop run [^ ]+ --$/,
      `${scriptName} should forward root npm arguments to the desktop script`,
    );
  }

  assert.equal(rootPackage.scripts["eval:report"], "npm --prefix apps/desktop run eval:report --");
  assert.equal(
    rootPackage.scripts["eval:report:latest"],
    "npm --prefix apps/desktop run eval:report:latest --",
  );
});

test("CI workflow avoids runner context in job-level env", () => {
  const workflow = read(".github/workflows/ci.yml");
  const invalidJobEnvExpressions = workflow.match(
    /^ {6}[A-Z][A-Z0-9_]*:\s*\$\{\{\s*runner\./gm,
  );

  assert.deepEqual(
    invalidJobEnvExpressions,
    null,
    "runner context is unavailable while GitHub evaluates job-level env",
  );
});

test("Desktop release workflow is manual or tag gated", () => {
  const workflow = read(".github/workflows/desktop-release.yml");

  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /tags:/);
  assert.match(workflow, /desktop-v\*/);
  assert.match(workflow, /runs-on:\s*macos-latest/);
  assert.match(workflow, /npm run tauri:build/);
  assert.match(workflow, /upload-artifact/);
});

test("CI shares fail-closed release inputs and immutable candidate artifacts", () => {
  const workflow = read(".github/workflows/ci.yml");
  const representativeHelper = read("scripts/build-representative-evidence.mjs");

  for (const path of [
    "scripts/\\*\\*",
    "release/\\*\\*",
    "docs/release/\\*\\*",
    "docs/superpowers/specs/2026-07-10-public-beta-convergence-design.md",
    "docs/superpowers/plans/2026-07-10-main-integration-release-truth.md",
    "package.json",
    "apps/desktop/package-lock.json",
    "apps/desktop/src-tauri/Cargo.lock",
    "apps/eval-runner/uv.lock",
    "apps/website/package-lock.json",
    "\\.github/workflows/desktop-release.yml",
  ]) {
    assert.match(workflow, new RegExp(path), `CI path filters should include ${path}`);
  }

  const releaseContract = workflow.slice(
    workflow.indexOf("release-contract:"),
    workflow.indexOf("desktop-frontend:"),
  );
  assert.match(releaseContract, /needs:\s*workflow-contract/);
  assert.match(releaseContract, /build-release-candidate\.test\.mjs/);
  assert.match(releaseContract, /validate-release-gate-profile\.test\.mjs/);
  assert.match(releaseContract, /validate-release-manifest\.test\.mjs/);
  assert.match(releaseContract, /release-confidence-summary\.test\.mjs/);
  assert.match(releaseContract, /forge-release-contract-\$\{\{ github\.sha \}\}/);
  assert.match(releaseContract, /if-no-files-found:\s*error/);

  const nightlySection = workflow.slice(
    workflow.indexOf("nightly-eval:"),
    workflow.indexOf("release-candidate:"),
  );
  assert.match(nightlySection, /--require-state R2/);
  assert.match(nightlySection, /eval-gate-results\.json/);
  assert.match(nightlySection, /build-representative-evidence\.mjs/);
  assert.match(nightlySection, /representative-mock\.json/);
  assert.match(nightlySection, /representative-real-forge\.json/);
  assert.match(nightlySection, /forge-eval-inputs-\$\{\{ github\.sha \}\}/);
  assert.match(nightlySection, /if-no-files-found:\s*error/);
  assert.doesNotMatch(nightlySection, /Skipping real smoke/);

  const candidateSection = workflow.slice(workflow.indexOf("release-candidate:"));
  assert.match(candidateSection, /github\.ref == 'refs\/heads\/main'/);
  assert.match(candidateSection, /scripts\/acceptance\.sh --release-profile release\/release-gates\.v1\.json --require-state R3/);
  assert.match(candidateSection, /npm run release:candidate/);
  assert.match(candidateSection, /forge-r3-\$\{\{ github\.sha \}\}/);
  assert.match(candidateSection, /candidate-manifest\.json/);
  assert.match(candidateSection, /if-no-files-found:\s*error/);

  assert.match(representativeHelper, /condition_status:\s*"passed"/);
  assert.doesNotMatch(workflow, /APPLE_(CERTIFICATE|CERTIFICATE_PASSWORD|SIGNING_IDENTITY|ID|PASSWORD)/);
});
