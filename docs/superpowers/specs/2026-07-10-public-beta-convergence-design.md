# Forge Public Beta Convergence Design

Date: 2026-07-10
Status: pending user review
Scope: Forge monorepo convergence from the current feature branch to a signed, notarized, publicly downloadable macOS beta

## Goal

Forge should move from a capable internal beta to a public macOS beta that is safe to run against an unfamiliar local repository, honest about evaluation evidence, reproducible from `main`, trusted by macOS Gatekeeper, and downloadable through the product website.

The completion target is:

> The current feature work is safely integrated into `main`, every public-beta blocker is closed with machine-readable evidence, and a new user can download, install, launch, quit, reopen, and continue using a signed and notarized Forge DMG without bypassing Gatekeeper.

This is a convergence program. It does not add another provider, runtime owner, product surface, or autonomous execution mode.

## Baseline

This design starts from commit `f5863df1e6fcbde55a9b4b2ceeacd9e3c354d3c3` on branch `cabbos/forge-feishu-upgrade-sync-hook`.

At the 2026-07-10 review baseline:

- The branch is 82 commits ahead of local `main`.
- The branch changes 143 files with 29,862 insertions and 497 deletions.
- Clean dependency installs can build both desktop and website applications.
- Eval Runner passes 189 of 189 pytest tests.
- Desktop mocked Playwright acceptance passes 40 of 40 scenarios.
- The CI-default acceptance subset passes 40 of 40 executable gates.
- Desktop frontend architecture checks pass 27 of 27 tests.
- All 45 Rust `StreamEvent` discriminants are handled by the frontend protocol layer.
- The full Rust backend gate passes formatting and Clippy but fails one of 1,733 tests because an old memory test still asserts a pre-unification ID.
- The Vite 8 desktop build completes with 66 unknown Tailwind-style at-rule warnings and leaves those directives in the generated CSS.
- Desktop acceptance emits one React Query error because a continuity query returns `undefined` in a mocked path.
- The current GitNexus index is 72 commits behind the branch; the prescribed refresh command times out after 60 seconds.

The baseline also contains three public-beta blockers:

1. Desktop treats project-defined build and test commands as read-only even though they execute arbitrary project code through the host shell.
2. Eval Runner can lose `provider`, `model`, and `case_source` when restoring a queued SQLite run, silently turning a requested Forge run into a mock run.
3. The repository has no complete signed, notarized, versioned, installation-tested public distribution path.

## Product Decision

The selected strategy is risk-first release convergence:

1. Close Desktop execution and secret-handling blockers.
2. Make Eval Runner a trustworthy source of release evidence.
3. Integrate the branch into `main` through reviewable, independently green slices.
4. Build one release-truth pipeline shared by CI and release workflows.
5. Add signing, notarization, installation proof, and a real website download only after the release candidate is green.

Two alternatives were rejected:

- Artifact-first release was rejected because it could produce a downloadable application before its execution boundary and evaluation evidence are trustworthy.
- Full parallel delivery was rejected because an 82-commit integration gap makes cross-stream conflicts and false-green evidence too likely.

## Program Decomposition

The program is split into four independently testable subprojects. Each subproject receives its own implementation plan after this design is approved.

### A. Desktop Safety Baseline

Purpose:

- Make execution against an unfamiliar repository fail closed.
- Move secrets out of ordinary configuration files.
- Prevent sensitive request and workspace content from entering persistent logs.
- Make checkpoint and restore behavior preserve the work the user believes is protected.

Primary authority:

- Rust backend policy, credential, logging, and checkpoint modules.

Exit condition:

- No open P0 in command execution, secret storage, log persistence, workspace boundaries, or checkpoint restoration.

### B. Eval Trustworthiness Baseline

Purpose:

- Ensure a requested provider and model are the provider and model actually executed.
- Derive workspace effects independently instead of trusting runner-reported file lists.
- Connect trust gates to CLI and API completion status.
- Make queued service execution authenticated, bounded, and protected from stale workers.

Primary authority:

- Persisted run metadata plus independently observed workspace and process evidence.

Exit condition:

- Eval Runner cannot silently downgrade, omit scope violations, or report a trusted release result when required evidence is missing.

### C. Main Integration And Release Truth

Purpose:

- Reduce the feature branch to a sequence of reviewable merge slices.
- Make local, pull-request, scheduled, and release gates use the same source of truth.
- Eliminate known red tests, build warnings, browser console errors, and stale gate semantics.
- Produce an immutable release-evidence manifest for one commit.

Primary authority:

- Machine-readable gate results bound to a commit SHA and dependency-lock hashes.

Exit condition:

- `main` builds reproducibly from clean installs and generates a release candidate manifest with no blocker, unknown required gate, or unreviewed residual P0.

### D. Public Beta Distribution

Purpose:

- Sign and notarize the macOS application and DMG.
- Validate the final artifact on a clean macOS user environment.
- Publish a versioned download with checksum and rollback evidence.
- Replace the website prototype CTA with a verified release-manifest-backed download.

Primary authority:

- Signed artifact identity, Apple notarization ticket, checksums, installation smoke evidence, and the immutable release manifest.

Exit condition:

- A new user can download and run Forge without a Gatekeeper override, and the website never points to an unverified artifact.

## Dependency Order

Desktop Safety and Eval Trustworthiness may run in parallel because they own different files and evidence domains.

Main Integration can prepare its merge topology, CI contracts, and evidence schema immediately, but it cannot declare a release candidate until both safety baselines are green.

Public Beta Distribution begins only after the release candidate is bound to a commit on `main`.

The dependency order is:

```text
Desktop Safety Baseline ---------+
                                 +--> Main Integration And Release Truth --> Public Beta Distribution
Eval Trustworthiness Baseline ---+
```

## Release State Machine

Forge public-beta readiness uses five states.

### R0: Baseline Frozen

Required evidence:

- Commit and branch identity.
- Current build, test, acceptance, audit, and known-risk evidence.
- Explicit public-beta blockers.

Current status: complete for the 2026-07-10 baseline.

### R1: Safety Baseline Green

Required evidence:

- Project-defined scripts are never silently treated as read-only.
- Unknown shell risk requires an explicit user decision.
- Catastrophic and external-write blocks remain effective in every permission mode.
- Provider credentials are stored through a system credential abstraction.
- Persistent logs redact credentials, authorization headers, request bodies, environment values, and hidden context.
- Checkpoint restore proves staged, unstaged, untracked, and binary-file behavior or explicitly refuses unsupported states without destructive cleanup.
- Desktop security regression tests and the complete backend gate pass.

### R2: Evaluation Truth Green

Required evidence:

- SQLite and in-memory storage round-trip every run execution field.
- Unknown providers are rejected instead of mapped to mock.
- A queued Forge run can be proven to execute Forge or fail explicitly.
- Workspace file effects are independently observed.
- Trust gates run in CLI, synchronous API, and queued worker paths.
- Missing evidence makes the release result unknown or failed, never passed.
- API access is authenticated for non-loopback use.
- Setup and validation commands have timeouts and cancellation boundaries.
- Worker claims use fencing tokens and reject stale completion writes.

### R3: Release Candidate

Required evidence:

- The candidate commit exists on `main`.
- Clean dependency installs reproduce desktop, Eval Runner, and website gates.
- Desktop backend, frontend architecture, protocol, unit, mocked browser, and packaged-app gates are green.
- Eval trust gates are green on representative mock and real-Forge evidence.
- CI observes changes to applications, scripts, workflows, release contracts, and relevant documentation.
- GitNexus is refreshed or the release artifact contains an explicit fallback impact report with residual risk.
- The release-evidence manifest is complete and internally consistent.

### R4: Public Beta

Required evidence:

- The application and DMG use the intended Developer ID identity.
- Hardened Runtime and required entitlements are explicit.
- Apple notarization succeeds and the ticket is stapled.
- `codesign`, `spctl`, stapler validation, and checksum verification pass on the final bytes.
- A clean macOS user completes download, install, first launch, permission refusal and approval, quit, reopen, and session recovery without a Gatekeeper bypass.
- The release tag and website download resolve to the same manifest and checksum.
- The previous beta remains available for rollback.

## Gate Semantics

All required gates fail closed.

- A gate that reports a blocker exits nonzero.
- A gate that did not run is `unknown`, not `passed`.
- Manual evidence is allowed only for OS permissions, installation, restart, and visual checks that cannot be automated reliably.
- Manual evidence cannot replace unit, integration, security, or artifact verification.
- No P0 may be waived by documentation alone.
- A release state advances only when all required evidence for that state is attached to the same commit.

The existing acceptance matrix may continue to include diagnostic commands whose purpose is to report a condition. Release confidence must distinguish `command_executed` from `condition_passed` so a zero exit code cannot hide a dirty or incomplete reported state.

## Authority Boundaries

### Desktop Execution Decisions

The Rust backend is authoritative. The frontend may request, display, and record a decision but cannot upgrade a denied, blocked, or confirmation-required operation to allowed.

Project-defined package scripts, test runners, compilers with build hooks, and equivalent commands are executable project code. They are not read-only merely because their conventional purpose is testing or building.

If Forge cannot prove a command safe, the decision is confirmation-required. A future sandbox may create a separate policy class, but public Beta does not depend on building a general-purpose sandbox.

### Credentials And Logs

The system credential store is authoritative for secret values. On macOS, the first implementation targets Keychain through a narrow credential-store interface.

Forge configuration may retain provider IDs, model IDs, non-secret endpoints, and credential references. It must not retain plaintext provider secrets as the normal post-migration state.

Log redaction happens before persistence. Call sites must not rely on every consumer remembering to redact a value independently.

### Evaluation Runs

Persisted run metadata is authoritative for requested execution identity. Independently collected process and workspace evidence is authoritative for observed execution effects.

Runner payloads remain useful evidence, but self-reported `changed_files`, provider labels, verification status, or success flags cannot be the only authority for a release gate.

### Release Artifacts

The immutable release manifest is authoritative for public distribution. It binds one commit to one set of lockfiles, gate results, artifact bytes, signing identity, notarization ticket, installation evidence, and website download URL.

The website consumes a verified manifest. It does not discover or promote the most recent CI artifact by timestamp.

## Failure Handling

### Desktop

- Unknown command risk becomes an explicit confirmation request.
- Credential-store failure prevents the affected provider from starting and presents a recovery path.
- Redaction failure prevents sensitive structured payloads from being persisted.
- Unsupported checkpoint states return a non-destructive error before restore mutates the worktree.

### Eval Runner

- Unknown provider input returns a client error.
- Missing or incompatible persisted execution fields stop claim or execution with a traceable failure.
- A lost or expired lease prevents the worker from publishing a terminal result.
- Timeout and cancellation produce explicit categories and preserve partial evidence.
- Missing independent workspace evidence prevents scope-sensitive trust gates from passing.

### Release Pipeline

- Any required unknown or failed gate prevents release-candidate promotion.
- Signing or notarization failure preserves the candidate as R3 but prevents R4 publication.
- Apple service unavailability never changes the website download.
- Installation smoke failure preserves the previous public beta and attaches diagnostic evidence to the failed candidate.
- Website publication happens only after final artifact verification and can roll back to the previous manifest without rebuilding the old artifact.

## Release Evidence Manifest

The release manifest is versioned and machine-readable. Its first version contains these logical sections:

```json
{
  "schema_version": 1,
  "version": "desktop-v0.1.0-beta.1",
  "commit_sha": "full git sha",
  "source_branch": "main",
  "lockfiles": [],
  "gates": [],
  "eval_evidence": [],
  "artifacts": [],
  "signing": {},
  "notarization": {},
  "installation_smoke": {},
  "website": {},
  "residual_risks": [],
  "generated_at": "ISO-8601 timestamp"
}
```

The implementation plan will define the exact schema, validation rules, and file paths. No field that contributes to release readiness may be inferred from prose.

## Verification Strategy

### Unit And Contract Tests

Desktop coverage includes:

- Command classification for project-defined scripts, shell control, external paths, and catastrophic commands.
- Permission-mode behavior for manual, trusted-project, and full-access modes.
- Credential create, read, migrate, delete, and unavailable-store behavior.
- Central log redaction for headers, tokens, request bodies, environment values, and hidden context.
- Checkpoint capture and restore for staged, unstaged, untracked, renamed, deleted, and binary files.

Eval coverage includes:

- Storage round-trip for every `EvaluationRun` execution field.
- Strict provider validation and no-fallback behavior.
- Independent workspace observation when runner evidence is absent or false.
- Trust-gate invocation across CLI, sync API, and queued worker paths.
- Authentication, timeout, cancellation, lease expiry, reclaim, and stale-worker completion.

Website and release coverage includes:

- Manifest schema validation.
- Selection of only a verified current release.
- Checksum and URL consistency.
- Safe rollback to a previous verified release.

### Integration Tests

- A malicious package script, Rust build hook, and test command prove that Desktop does not silently execute project-defined code.
- A queued SQLite Forge run proves that execution identity survives persistence and claim.
- A fixture that omits or falsifies `changed_files` proves independent scope detection.
- Clean dependency installations precede production builds.
- CI and release workflows consume the same release-confidence contract.

### Repository Gates

The repository gate set covers:

- Desktop backend formatting, Clippy, and all Rust tests.
- Desktop frontend architecture and protocol checks.
- Frontend unit and mocked Playwright acceptance tests.
- Eval Runner tests, formatting, lint, type checks, and trust-gate smoke.
- Website build, accessibility smoke, and download-manifest contract.
- Acceptance matrix contract and CI-default runtime gates.
- Release manifest validation.

### Packaged macOS Evidence

The packaged-app proof covers:

- Final DMG bytes, not an earlier unsigned build.
- Signature identity and nested-code verification.
- Notarization and stapling.
- Gatekeeper assessment.
- Clean-user install and first launch.
- Permission deny and allow paths.
- Quit and reopen recovery.
- One real provider readiness and bounded task smoke without logging secrets.
- Uninstall or cleanup instructions and previous-version rollback availability.

## Merge Strategy

The 82-commit branch gap is integrated as a merge train, not one opaque pull request.

The planned dependency order is:

1. Test and CI contract repairs that do not change runtime behavior.
2. Desktop security baseline.
3. Eval persistence and trust baseline.
4. Runtime and evidence work already proven by the CI-default gates.
5. Frontend surfaces that consume those backend contracts.
6. Release manifest and workflow changes.
7. Website download integration and public-beta documentation.

Each slice must build and pass its relevant gates on top of the preceding slice. A later slice cannot be used to make an earlier slice appear green.

## Rollout Phases

### Phase 1: Stop The Bleeding

- Fix the deterministic backend test failure.
- Remove build warning and browser-console noise from required gates.
- Close Desktop command-execution, secret, log, CSP, and checkpoint P0/P1 findings.

### Phase 2: Make Evaluation Honest

- Repair persistence and strict provider identity.
- Add independent workspace evidence.
- Wire trust gates into every execution path.
- Add authentication, timeouts, cancellation, and fencing.

### Phase 3: Converge On Main

- Create and execute the merge train.
- Refresh code intelligence or attach the required fallback evidence.
- Make CI and release-confidence semantics fail closed.
- Produce the first R3 manifest from `main`.

### Phase 4: Build The Trusted Artifact

- Enroll in Apple Developer Program when signing credentials are needed.
- Configure Developer ID, Hardened Runtime, entitlements, notarization, stapling, and verification.
- Run packaged macOS smoke on the final artifact.

### Phase 5: Publish The Beta

- Create a versioned tag and immutable artifact location.
- Publish checksum, system requirements, privacy and security notes, and rollback link.
- Switch the website CTA through the verified release manifest.
- Monitor first-install failures without collecting prompt or workspace content.

## Documentation Contract

User-visible behavior changes remain synchronized across:

- Root `README.md`.
- `apps/desktop/README.md`.
- `apps/eval-runner/README.md` when evaluation behavior changes.
- `CHANGELOG.md`.
- Product beta and release evidence documents.
- Acceptance gate metadata and release manifest schema.

Documentation cannot mark a release state ahead of machine-readable evidence.

## Non-Goals

The first public-beta convergence program does not include:

- Mac App Store submission.
- Apple Developer Enterprise Program.
- Windows or Linux public distribution.
- An automatic updater; the first beta uses versioned DMGs with an explicit rollback download.
- New model providers or provider transports.
- A new gateway owner or broader autonomous execution.
- New product surfaces unrelated to public-beta blockers.
- A large website redesign.
- Extraction of shared packages before two applications actually share implementation code.
- Automatic commit, merge, push, or release promotion by the Forge runtime.

## Completion Standard

The program is complete only when:

- The approved merge train is integrated into `main`.
- No public-beta P0 remains.
- R1, R2, R3, and R4 evidence is complete for the same release lineage.
- All required gates pass from clean environments.
- Eval Runner cannot silently downgrade execution or accept self-reported scope as sole truth.
- The final DMG is signed, notarized, stapled, Gatekeeper-accepted, and installation-tested.
- The website serves the verified release and checksum.
- A previous beta remains available for rollback.
- Residual P1/P2 risks are explicit in the release manifest and user-facing release notes.

## Review Gate

This document defines the public-beta convergence design. After user review, the next step is to create separate task-by-task implementation plans for:

1. Desktop Safety Baseline.
2. Eval Trustworthiness Baseline.
3. Main Integration And Release Truth.
4. Public Beta Distribution.

Implementation begins with the two independent safety baselines and preserves the release-state dependencies defined here.
