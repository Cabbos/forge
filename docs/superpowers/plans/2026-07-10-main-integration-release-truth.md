# Main Integration And Release Truth Implementation Plan

> **Execution rule:** follow this plan task-by-task. Keep each checkbox as a reviewable merge slice. Before changing any symbol, run the GitNexus impact query listed in the task; warn before HIGH/CRITICAL results. If the index is stale, record the attempted refresh, exact error/timeout, indexed/current commits, symbols and files searched, direct callers/processes found manually, selected tests, affected authority domains, and residual risk in the candidate manifest. Run `detect_changes({scope:"compare",base_ref:"main"})` immediately before every task commit.

**Goal:** integrate the current feature branch into `main` through independently green slices and produce a fail-closed, commit-bound R3 release manifest consumed by the signing/distribution plan.

**Design authority:** `docs/superpowers/specs/2026-07-10-public-beta-convergence-design.md`.

**Shared contract with other plans:**

- Desktop Safety and Eval Trustworthiness own runtime/security behavior; this plan only consumes their machine-readable evidence.
- Public Beta Distribution consumes `release/release-manifest.schema.json`, `scripts/validate-release-manifest.mjs`, and `release/evidence/$TAG/candidate-manifest.json` produced here.
- No release state advances on prose alone. Missing or unexecuted required evidence is `unknown`.

## Task 1: Freeze R0 and define the merge-train contract

**Files:** `docs/superpowers/specs/2026-07-10-public-beta-convergence-design.md`, `docs/superpowers/plans/2026-07-10-main-integration-release-truth.md`, `release/evidence/2026-07-10-r0-baseline.json`, `scripts/release-baseline.test.mjs`.

- [ ] Write the baseline JSON with the frozen design baseline commit (`f5863df1e6fcbde55a9b4b2ceeacd9e3c354d3c3`), the observed commit used for planning, `main`, branch divergence, lockfile hashes, current gate counts, known red tests, warning counts, and the three named public-beta blockers. Do not replace the frozen baseline with a later HEAD.
- [ ] Leave the design status as `pending user review` until the user's explicit approval is recorded; then update it with the approval date and links to the baseline JSON and four plans. This task must not self-approve the design.
- [ ] Add a Node contract test that validates required R0 keys, full SHA format, non-empty lockfile hashes, and explicit blocker IDs.
- [ ] **Red:** run `node --test scripts/release-baseline.test.mjs` before the fixture exists; it must fail for the missing file/keys.
- [ ] **Green:** create the fixture from `git rev-parse`, `git rev-list`, `git diff --stat`, `sha256sum`, and the existing acceptance/eval reports, then run the test and verify it passes.
- [ ] Commit as `docs(release): freeze public beta convergence baseline`.

## Task 2: Specify the Desktop Safety handoff (execute after Task 5)

**Files:** `release/evidence/merge-train/desktop-safety.json`, `scripts/release-gate-profile.test.mjs`, `docs/release/merge-train-2026-07-10.md`.

- [ ] Do not edit Desktop Rust, CSS, IPC fixtures, credentials, redaction, or checkpoint code in this plan; those changes belong exclusively to `2026-07-10-desktop-safety-baseline.md`.
- [ ] Require the five fixed Desktop Safety acceptance labels and their result artifacts in the R1 handoff. The handoff must include the deterministic memory-ID test, Tailwind 4 warning-free build, and continuity fixture evidence produced by the owning plan.
- [ ] **Red:** after Task 5 creates the profile validator, validate a handoff missing one label or containing a failed condition; the release profile must reject it.
- [ ] **Green:** after Task 5, validate the complete Desktop Safety handoff and record its producing commit SHA.
- [ ] Commit as `docs(release): consume desktop safety handoff contract`.

## Task 3: Specify the Eval Trustworthiness handoff (execute after Task 5)

**Files:** `release/evidence/merge-train/eval-trustworthiness.json`, `scripts/release-gate-profile.test.mjs`, `docs/release/merge-train-2026-07-10.md`.

- [ ] Require the four fixed Eval acceptance labels and their result artifacts in the R2 handoff. The handoff must prove strict provider identity, independent workspace observation, trusted orchestration, and authenticated/fenced worker behavior, plus the full Eval quality suite (`uv run pytest -q`, `uv run ruff check .`, `uv run ruff format --check .`, and `uv run mypy app`); focused tests alone cannot claim R2.
- [ ] **Red:** after Task 5 creates the profile validator, validate a handoff with an unknown provider, missing workspace evidence, or stale-worker failure; the release profile must reject it.
- [ ] **Green:** after Task 5, validate the complete Eval Trustworthiness handoff and record its producing commit SHA.
- [ ] Commit as `docs(release): consume eval trustworthiness handoff contract`.

## Task 4: Bind deterministic signal evidence to its owning slice

**Files:** `docs/release/merge-train-2026-07-10.md`, `scripts/release-gate-profile.test.mjs`.

- [ ] Record the unified-memory assertion fix, Tailwind 4 warning-free build, and console-clean continuity fixture only as Desktop Safety evidence; do not introduce a competing implementation here.
- [ ] **Green:** require the owning slice's evidence paths and commit SHA in the R1 profile, then run the profile contract test.
- [ ] Commit as `docs(release): bind deterministic signal evidence to owner slices`.

## Task 5: Define the shared release-gate and manifest schemas first

**Files:** `release/release-gate-profile.schema.json`, `release/release-gates.v1.json`, `release/release-manifest.schema.json`, `scripts/validate-release-manifest.mjs`, `scripts/validate-release-manifest.test.mjs`, `scripts/release-gate-profile.test.mjs`.

- [ ] Define gate states separately: `execution_status` (`not_started`, `running`, `completed`, `execution_failed`) and `condition_status` (`passed`, `failed`, `manual`, `unknown`). Preserve the legacy `status` field as a compatibility alias to `condition_status` for one migration window.
- [ ] Define the profile fields `id`, `required_for` (`R1`–`R4`), `label`, `domain`, `tier`, `manual_allowed`, `command`, `evidence_schema`, and `ci_default`. A gate is required only when explicitly present in the profile.
- [ ] Implement `scripts/validate-release-gate-profile.mjs` and its test. It must require the fixed five Desktop Safety labels, four Eval Trustworthiness labels, and four repository/distribution contract labels; reject duplicate labels, missing R3 entries, and any acceptance label that is not explicitly classified. Add `--release-profile release/release-gates.v1.json --require-state R3` as the only supported R3 selection path across acceptance, summary, and manifest commands.
- [ ] Define manifest schema version 1 with `version`, `release_state`, `generated_at`, commit SHA, source branch, lockfile hashes, profile ID, selected gates, gate results, Eval evidence, artifacts, signing, notarization, installation smoke, website, previous release, GitNexus impact/fallback evidence, and residual risks. Require explicit `unknown` entries for selected gates that did not run.
- [ ] Write validator tests for missing fields, duplicate gate labels, selected-vs-executed mismatch, failed/unknown required gates, mismatched commit SHA, missing lockfile hash, and a fully valid R3 fixture.
- [ ] **Red:** run both test files before implementing validators; the invalid fixtures must fail validation.
- [ ] Implement pure validation functions with no network calls. The CLI must return 2 for malformed schema and 1 for a structurally valid manifest that fails its required release state.
- [ ] **Green:** run the two test files and validate `release/evidence/fixtures/r3-valid.json`.
- [ ] Commit as `feat(release): add fail-closed gate and manifest contracts`.

## Task 6: Make acceptance results distinguish execution from conditions

**Files:** `scripts/acceptance.sh`, `scripts/acceptance.test.mjs`, `scripts/release-confidence-summary.mjs`, `scripts/release-confidence-summary.test.mjs`.

- [ ] Run GitNexus impact for `record_gate_result`, `write_results_json`, `buildReleaseConfidenceSummary`, `summarizeGateExecution`, and `summarizeStatus` before editing.
- [ ] Extend `record_gate_result` and `write_results_json` to emit schema version 2 with both statuses, `startedAt`, `finishedAt`, `exitCode`, `reason`, and the gate profile ID. Keep the old fields for consumers that have not migrated.
- [ ] Treat a gate that was selected but never entered as `condition_status: unknown`; a command that starts and exits nonzero is `execution_status: completed, condition_status: failed` unless the shell cannot start (`126`/`127`), which is `execution_failed, condition_status: unknown`.
- [ ] Add strict command variants for report-producing required gates (`--require-ready`, `--require-live-ready`, validator `--require-complete`) so a zero exit code cannot hide a reported blocker. Record the reason field from the report when available.
- [ ] **Red:** add fixtures with a selected gate count greater than executed count and with a diagnostic command returning zero while reporting `blocked`; assert the current summary incorrectly passes them.
- [ ] **Green:** make the summary consume `condition_status` first, calculate execution completeness separately, and return `failed` for required failed conditions, `attention_required` for required unknown/manual evidence, and `passed` only when all gates selected by `--release-profile release/release-gates.v1.json --require-state R3` are passed.
- [ ] Add tests for `--no-acceptance-matrix`, `--ci-default-only`, `--fail-on-attention`, dashboard artifacts, domain/tier breakdowns, and backward-compatible v1 results.
- [ ] Commit as `fix(eval): make release confidence fail closed on incomplete gates`.

## Task 7: Implement the immutable R3 candidate-manifest generator

**Files:** `scripts/build-release-candidate.mjs`, `scripts/build-release-candidate.test.mjs`, `release/evidence/$TAG/candidate-manifest.json`, `scripts/acceptance.sh`, `package.json`, `.github/workflows/ci.yml`.

- [ ] Implement `build-release-candidate.mjs` to require a clean Git tree, full commit SHA, profile ID, lockfile hashes, acceptance results, Eval Trust evidence, Desktop Safety evidence, and the GitNexus fallback/refresh record.
- [ ] Require an explicit `--tag desktop-vX.Y.Z-beta.N` and require the candidate commit to be reachable from integrated `main`; bind every input by SHA-256 and write the manifest atomically under `release/evidence/$TAG/candidate-manifest.json`; refuse overwrite unless `--replace-existing` is explicitly used for a still-R3 candidate with the same commit.
- [ ] Upload the exact candidate JSON as the immutable CI artifact `forge-r3-<full-sha>`, where `<full-sha>` is the 40-character candidate commit SHA. Public Beta Distribution must resolve this artifact by tag target SHA, never by newest-run lookup.
- [ ] **Red:** test that missing safety/eval evidence, dirty worktree, stale selected gate count, or a different commit SHA exits nonzero and writes no candidate.
- [ ] **Green:** generate a local structural fixture without declaring R3, then validate the generator's input/output contract. Do not produce a release candidate from the feature branch; actual R3 generation happens only after Task 9 confirms the integrated commit is reachable from `main`.
- [ ] Add root scripts `release:candidate` and `release:validate` that call these exact helpers; no CI job should reimplement manifest fields.
- [ ] Commit as `feat(release): implement commit-bound candidate generator`.

## Task 8: Make CI observe the release contract and the real source paths

**Files:** `.github/workflows/ci.yml`, `.github/workflows/desktop-release.yml`, `scripts/ci-workflow.test.mjs`, `scripts/acceptance.sh`.

- [ ] Expand CI path filters to include `scripts/**`, `release/**`, both workflow files, root lockfiles/package metadata, and the documentation that defines a gate or release contract.
- [ ] Add a `release-contract` job that installs Node, runs schema/acceptance/release-confidence contract tests, and emits the candidate input artifacts without signing or publishing.
- [ ] Make nightly and manual eval jobs upload the same gate-results JSON, Eval report, and boundary evidence consumed by `build-release-candidate.mjs`; do not synthesize a green result when an artifact is absent.
- [ ] Add workflow tests for path coverage, required job dependencies, artifact names, `condition_status`, and the fact that Apple credentials are absent from ordinary CI.
- [ ] **Red:** run `node --test scripts/ci-workflow.test.mjs` before workflow edits and record the missing contract assertions.
- [ ] **Green:** run `npm run check:ci`, `scripts/acceptance.sh --dry-run`, and the release-contract job locally through its command list.
- [ ] Commit as `ci(release): share release truth across CI and candidate builds`.

## Task 9: Execute the merge train and bind evidence to main

**Files:** `docs/superpowers/plans/2026-07-10-main-integration-release-truth.md`, `release/evidence/merge-train/<slice>.json`, `docs/release/merge-train-2026-07-10.md`.

- [ ] Create one branch/commit per completed task in this order: contract repairs, Desktop Safety, Eval Trustworthiness, runtime/evidence consumers, frontend consumers, release contract, distribution.
- [ ] For every slice, record parent SHA, child SHA, changed files, GitNexus impact result or fallback, targeted Red/Green commands, full relevant gate output paths, and residual risk. A later slice cannot update an earlier slice’s evidence.
- [ ] Before each shared runtime/API change, run `mcp__gitnexus__impact` with `direction: "upstream"`, `includeTests: true`, and the exact file/symbol. Before every slice commit and again before the candidate, run `mcp__gitnexus__detect_changes` with `scope: "compare"`, `base_ref: "main"`; if refresh times out, attach the fallback report and set residual risk to `HIGH`.
- [ ] Merge only slices whose required gates are green. Resolve conflicts by preserving the owning plan’s authority boundary; do not merge generated artifacts or credentials.
- [ ] **Red:** run the full acceptance matrix from a clean install before the first merge slice and retain the known failures.
- [ ] **Green:** after each slice run its targeted tests; after the final slice run clean app-level `npm ci`, `uv sync --dev`, Cargo checks, `npm run check:ci`, `npm run build:desktop`, `npm run build:website`, `npm run test:eval`, and the profile-selected R3 acceptance run (not the unrestricted matrix and not only `--ci-default`).
- [ ] Once the integrated commit is on `main`, run `scripts/build-release-candidate.mjs --tag "$TAG" --release-profile release/release-gates.v1.json --require-state R3`, upload the resulting JSON as `forge-r3-<full-sha>`, and record the artifact URL/SHA in the merge-train evidence.
- [ ] Commit the merge-train record as `docs(release): record main integration evidence`.

## Task 10: Handoff R3 to Public Beta Distribution

**Files:** `release/evidence/$TAG/candidate-manifest.json`, `docs/public-beta.md`, `README.md`, `apps/desktop/README.md`, `apps/eval-runner/README.md`, `CHANGELOG.md`.

- [ ] Verify the candidate manifest is valid at R3, contains no failed/unknown required gate, and includes explicit manual-evidence entries only for gates allowed by the profile.
- [ ] Update user-facing docs to state that public distribution is blocked until Developer ID signing, notarization, stapling, clean-user installation evidence, and a verified website manifest exist; do not claim R4 early.
- [ ] Hand the exact manifest path, commit SHA, lockfile hashes, and residual-risk list to the Public Beta Distribution plan. Distribution must not rebuild or reinterpret R3 evidence.
- [ ] Run `node scripts/validate-release-manifest.mjs --manifest release/evidence/$TAG/candidate-manifest.json --require-state R3` as the final handoff check.
- [ ] Commit as `docs(release): hand off verified R3 candidate`.

## Final verification commands

```bash
git status --short --branch
npm --prefix apps/desktop ci
npm --prefix apps/website ci
cd apps/eval-runner && uv sync --dev && cd ../..
npm run check:ci
npm run build:desktop
npm run build:website
npm run test:eval
cd apps/eval-runner && uv run pytest -q && uv run ruff check . && uv run ruff format --check . && uv run mypy app && cd ../..
node --test scripts/release-baseline.test.mjs scripts/release-gate-profile.test.mjs scripts/validate-release-manifest.test.mjs scripts/acceptance.test.mjs scripts/release-confidence-summary.test.mjs scripts/ci-workflow.test.mjs
scripts/acceptance.sh --dry-run
scripts/acceptance.sh --release-profile release/release-gates.v1.json --require-state R3 --results-json /tmp/forge-r3-gates.json
scripts/acceptance.sh --ci-default --results-json /tmp/forge-ci-default-gates.json
node scripts/validate-release-gate-profile.mjs --release-profile release/release-gates.v1.json --require-state R3
node scripts/release-confidence-summary.mjs --release-profile release/release-gates.v1.json --gate-results /tmp/forge-r3-gates.json --require-state R3 --fail-on-attention
node scripts/release-confidence-summary.mjs --gate-results /tmp/forge-ci-default-gates.json --ci-default-only --fail-on-attention
node scripts/validate-release-manifest.mjs --manifest release/evidence/$TAG/candidate-manifest.json --require-state R3
git diff --check
```

The plan is complete only when the R3 manifest validates from a clean checkout of the integrated `main` commit. Apple signing, notarization, clean-user install, website publication, and R4 promotion remain owned by `2026-07-10-public-beta-distribution.md`.
