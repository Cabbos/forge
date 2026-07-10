# Forge Public Beta Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish one universal, Developer ID-signed, notarized, stapled, clean-user-tested Forge macOS beta whose versioned DMG, checksum, rollback release, and website download are all bound to the same immutable R4 release manifest.

**Architecture:** Subproject C remains authoritative for the R3 schema, validator, and candidate manifest. The release job resolves the exact successful `forge-r3-<full-sha>` CI artifact for the tag target SHA, downloads it to `release/evidence/<tag>/candidate-manifest.json`, signs and notarizes one universal app/DMG lineage, validates the final bytes, requires structured clean-user evidence, then creates an immutable versioned release. The website uses a checked-in channel pointer to one byte-verified versioned manifest snapshot and fails closed when any signing, notarization, checksum, installation, or URL invariant is absent.

**Tech Stack:** Tauri 2.11, Rust/Cargo, Node.js 20 ESM and `node:test`, GitHub Actions/GitHub Releases, Apple `codesign`/`notarytool`/`stapler`/`spctl`, React 19/Vite 8, Playwright, axe-core.

---

## Fixed cross-project contracts

- C owns `release/release-manifest.schema.json` and `scripts/validate-release-manifest.mjs`.
- The R3 CI artifact name is exactly `forge-r3-<full-sha>`; `<full-sha>` means the 40-character tag target commit SHA.
- The release job downloads those bytes to `release/evidence/<tag>/candidate-manifest.json`. This file is generated in the release workspace and is not committed into the candidate commit, avoiding manifest self-reference.
- Candidate lookup is by exact tag target SHA, successful workflow conclusion, and exact artifact name. Selecting the newest run or newest artifact is forbidden.
- D writes final evidence under `release/evidence/<tag>/` and the final manifest at `release/evidence/<tag>/release-manifest.json`.
- C references these exact acceptance labels; they must not be renamed:
  - `macOS signing configuration contract`
  - `public beta artifact verification contract`
  - `public beta install evidence contract`
  - `website verified download contract`

## File map

### Release authority and workflow

- Modify `.github/workflows/desktop-release.yml`: exact-tag R3 resolution, protected Apple credentials, universal build, notarization, final verification, evidence download, manifest finalization, immutable release publication.
- Create `.github/workflows/macos-install-evidence.yml`: run the clean-user protocol on an explicitly identified staged build and upload the exact tag/SHA-bound evidence artifact.
- Modify `.github/workflows/ci.yml`: observe release workflow, release contracts, website tests, and public-beta documentation.
- Create `scripts/resolve-r3-candidate.mjs`: select exactly one successful CI workflow run/artifact for the tag target SHA and download it to the fixed handoff path.
- Create `scripts/resolve-r3-candidate.test.mjs`: reject newest-by-time lookup, wrong SHA, failed runs, ambiguous runs, and wrong artifact names.
- Create `scripts/release-version.mjs`: validate tag, tag target, `main` ancestry, manifest SHA, and desktop version consistency.
- Create `scripts/release-version.test.mjs`: cover valid beta tags and every mismatch.
- Create `scripts/finalize-public-release.mjs`: combine the R3 candidate, final artifact evidence, install evidence, website URL, and previous release into one R4 manifest.
- Create `scripts/finalize-public-release.test.mjs`: prove fail-closed R4 promotion and deterministic output.

### macOS artifact identity

- Modify `apps/desktop/src-tauri/tauri.conf.json`: make Hardened Runtime, entitlements, universal distribution assumptions, and macOS 14 minimum explicit.
- Create `apps/desktop/src-tauri/Entitlements.plist`: minimal empty hardened-runtime exception set.
- Create `scripts/macos-signing-config.test.mjs`: reject implicit Hardened Runtime and dangerous entitlement exceptions.
- Create `scripts/verify-macos-artifact.mjs`: inspect the mounted final DMG and emit normalized codesign, notarization, stapling, Gatekeeper, architecture, version, and checksum evidence.
- Create `scripts/verify-macos-artifact.test.mjs`: use injected command fixtures to verify parsing and all failure categories.

### Clean-user evidence

- Create `release/macos-install-smoke.schema.json`: machine-readable clean-user install/launch/deny/approve/quit/reopen/recovery evidence contract.
- Create `scripts/macos-install-smoke-preflight.mjs`: verify platform, final DMG/manifest identity, clean home, quarantine, and evidence capture prerequisites.
- Create `scripts/macos-install-smoke-preflight.test.mjs`: test clean and contaminated user states.
- Create `scripts/validate-macos-install-evidence.mjs`: bind manual evidence to final bytes and reject Gatekeeper bypasses or incomplete recovery.
- Create `scripts/validate-macos-install-evidence.test.mjs`: cover each required R4 installation fact.
- Create `apps/desktop/docs/product/public-beta-install-smoke.md`: exact clean-user protocol and evidence capture commands.

### Verified website download

- Create `apps/website/src/release.js`: fetch, hash, validate, and normalize the pinned local release manifest.
- Create `apps/website/src/release.test.mjs`: pure contract tests for ready and fail-closed states.
- Modify `apps/website/src/App.jsx`: replace all toast buttons with one verified release-backed link contract and publish checksum/system/security/rollback facts.
- Modify `apps/website/src/styles.css`: verified/unavailable CTA and release-detail styling with visible focus states.
- Modify `apps/website/index.html`: correct language, canonical, description, Open Graph, and robots metadata.
- Create `apps/website/public/robots.txt`: allow public indexing without inventing an undeclared production domain.
- Create `apps/website/playwright.config.mjs`: independent website smoke runner.
- Create `apps/website/e2e/download.spec.mjs`: SEO, accessibility, fail-closed, exact URL, and download HEAD/range smoke.
- Modify `apps/website/package.json` and `apps/website/package-lock.json`: add Node contract and Playwright/axe scripts and dependencies.
- Create during promotion `apps/website/public/releases/manifests/desktop-v0.1.0-beta.1.json`: byte-identical final manifest snapshot.
- Create during promotion `apps/website/public/releases/public-beta.json`: channel pointer containing the snapshot path and SHA-256.

### Repository gates and documentation

- Modify `scripts/ci-workflow.test.mjs`, `scripts/acceptance.sh`, and `scripts/acceptance.test.mjs`: advertise and enforce the four fixed labels.
- Modify `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md`: public distribution behavior and commands.
- Create `docs/public-beta.md`: macOS 14+, universal architecture, install/uninstall, privacy/security, checksum, known risks, and rollback.

## Task 1: Resolve the exact R3 candidate and lock version/tag identity

**Files:**
- Create: `scripts/resolve-r3-candidate.mjs`
- Create: `scripts/resolve-r3-candidate.test.mjs`
- Create: `scripts/release-version.mjs`
- Create: `scripts/release-version.test.mjs`

- [ ] **Step 1: Write failing exact-selection tests**

Export these functions from `resolve-r3-candidate.mjs`:

```js
export function expectedArtifactName(commitSha) {
  if (!/^[0-9a-f]{40}$/.test(commitSha)) throw new Error("commit SHA must be 40 lowercase hex characters");
  return `forge-r3-${commitSha}`;
}

export function selectSuccessfulRun({ commitSha, runs, artifactsByRun }) {
  const name = expectedArtifactName(commitSha);
  const matches = runs
    .filter((run) => run.head_sha === commitSha && run.status === "completed" && run.conclusion === "success")
    .flatMap((run) => (artifactsByRun[run.id] ?? [])
      .filter((artifact) => artifact.name === name && artifact.expired === false)
      .map((artifact) => ({ runId: run.id, artifactId: artifact.id, artifactName: artifact.name })));
  if (matches.length !== 1) throw new Error(`expected one successful ${name} artifact, found ${matches.length}`);
  return matches[0];
}

export async function downloadCandidate({ repo, commitSha, tag, outputPath, github }) {
  const runs = await github.listWorkflowRuns({ repo, headSha: commitSha });
  const artifactsByRun = Object.fromEntries(await Promise.all(runs.map(async (run) => [
    run.id,
    await github.listArtifacts({ repo, runId: run.id }),
  ])));
  const selected = selectSuccessfulRun({ commitSha, runs, artifactsByRun });
  await github.downloadArtifact({ repo, artifactId: selected.artifactId, outputPath });
  return { ...selected, tag, commitSha, outputPath };
}
```

Tests must include: exact success, wrong SHA, failed/cancelled run, expired artifact, wrong name, duplicate matching runs, duplicate matching artifacts, and a newer wrong-SHA run that must not be selected.

- [ ] **Step 2: Run the tests and verify red**

Run:

```bash
node --test scripts/resolve-r3-candidate.test.mjs scripts/release-version.test.mjs
```

Expected: FAIL because both modules and their exports do not exist.

- [ ] **Step 3: Implement exact R3 resolution and version validation**

`downloadCandidate` must use GitHub API results filtered by `head_sha === commitSha`, `status === "completed"`, and `conclusion === "success"`; it must then require exact artifact name `forge-r3-${commitSha}` and write to `release/evidence/${tag}/candidate-manifest.json`.

Export from `release-version.mjs`:

```js
export const DESKTOP_TAG = /^desktop-v(\d+\.\d+\.\d+-beta\.\d+)$/;
export function parseDesktopTag(tag) {
  const match = DESKTOP_TAG.exec(tag);
  if (!match) throw new Error("tag must match desktop-vX.Y.Z-beta.N");
  return { tag, appVersion: match[1] };
}

export function validateReleaseIdentity({
  tag,
  tagTargetSha,
  candidateManifest,
  tauriVersion,
  cargoVersion,
  npmVersion,
  tagTargetIsOnMain,
}) {
  const { appVersion } = parseDesktopTag(tag);
  const failures = [];
  if (tagTargetSha !== candidateManifest.commit_sha) failures.push("tag target SHA differs from candidate commit_sha");
  if (candidateManifest.source_branch !== "main") failures.push("candidate source_branch must be main");
  if (!tagTargetIsOnMain) failures.push("tag target is not reachable from origin/main");
  for (const [name, value] of Object.entries({ tauriVersion, cargoVersion, npmVersion })) {
    if (value !== appVersion) failures.push(`${name} ${value} differs from tag version ${appVersion}`);
  }
  if (failures.length > 0) throw new Error(failures.join("; "));
  return { tag, appVersion, commitSha: tagTargetSha, sourceBranch: "main" };
}
```

The CLI must read the three version files, call `git rev-list -n 1 <tag>` and `git merge-base --is-ancestor <sha> origin/main`, and exit nonzero on any mismatch.

- [ ] **Step 4: Run focused green tests**

Run:

```bash
node --test scripts/resolve-r3-candidate.test.mjs scripts/release-version.test.mjs
```

Expected: PASS; the newer wrong-SHA fixture remains rejected and ambiguous matches exit nonzero.

- [ ] **Step 5: Refactor and run the C handoff validator**

Run:

```bash
node scripts/validate-release-manifest.mjs \
  --manifest release/evidence/desktop-v0.1.0-beta.1/candidate-manifest.json \
  --require-state R3
node scripts/release-version.mjs \
  --tag desktop-v0.1.0-beta.1 \
  --manifest release/evidence/desktop-v0.1.0-beta.1/candidate-manifest.json \
  --require-main
```

Expected on a release workspace containing the downloaded candidate: both commands print `status: passed`, the same full SHA, and `0.1.0-beta.1`. On an ordinary checkout without generated evidence, the first command fails with the exact missing path and does not search for another manifest.

- [ ] **Step 6: Commit**

```bash
git add scripts/resolve-r3-candidate.mjs scripts/resolve-r3-candidate.test.mjs scripts/release-version.mjs scripts/release-version.test.mjs
git commit -m "feat(release): bind beta tags to exact R3 evidence"
```

## Task 2: Make Hardened Runtime and entitlements explicit

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/Entitlements.plist`
- Create: `scripts/macos-signing-config.test.mjs`

- [ ] **Step 1: Write the failing signing configuration contract**

The test must assert:

```js
assert.equal(config.bundle.macOS.hardenedRuntime, true);
assert.equal(config.bundle.macOS.entitlements, "Entitlements.plist");
assert.equal(config.bundle.macOS.minimumSystemVersion, "14.0");
assert.equal(config.bundle.targets, "dmg");
```

Parse the plist and reject true values for `com.apple.security.get-task-allow`, `com.apple.security.cs.disable-library-validation`, `com.apple.security.cs.allow-dyld-environment-variables`, `com.apple.security.cs.allow-unsigned-executable-memory`, and `com.apple.security.cs.disable-executable-page-protection`.

- [ ] **Step 2: Run red**

Run:

```bash
node --test scripts/macos-signing-config.test.mjs
```

Expected: FAIL because `hardenedRuntime`, `entitlements`, and the plist are absent.

- [ ] **Step 3: Add minimal production configuration**

Add to `bundle.macOS`:

```json
{
  "minimumSystemVersion": "14.0",
  "hardenedRuntime": true,
  "entitlements": "Entitlements.plist"
}
```

Create `Entitlements.plist` as an ASCII XML plist with an empty `<dict/>`. Do not commit a certificate identity; the protected release environment supplies `APPLE_SIGNING_IDENTITY`.

- [ ] **Step 4: Run green and plist validation**

Run:

```bash
plutil -lint apps/desktop/src-tauri/Entitlements.plist
node --test scripts/macos-signing-config.test.mjs
npm run build:desktop
```

Expected: plist `OK`, contract PASS, desktop production build exits 0.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/Entitlements.plist scripts/macos-signing-config.test.mjs
git commit -m "build(macos): declare hardened runtime entitlements"
```

## Task 3: Build a fail-closed final macOS artifact verifier

**Files:**
- Create: `scripts/verify-macos-artifact.mjs`
- Create: `scripts/verify-macos-artifact.test.mjs`

- [ ] **Step 1: Write injected-runner tests**

Define the output contract:

```js
{
  schema_version: 1,
  status: "passed",
  tag: "desktop-v0.1.0-beta.1",
  commit_sha: "0123456789abcdef0123456789abcdef01234567",
  dmg: { file_name: "Forge_0.1.0-beta.1_universal.dmg", size_bytes: 1, sha256: "a".repeat(64) },
  app: {
    bundle_id: "com.cabbos.forge",
    version: "0.1.0-beta.1",
    architectures: ["arm64", "x86_64"],
    signing_identity: "Developer ID Application: Forge Test (ABCDE12345)",
    team_id: "ABCDE12345",
    hardened_runtime: true,
    entitlements_sha256: "b".repeat(64)
  },
  notarization: { status: "accepted", submission_id: "11111111-2222-3333-4444-555555555555" },
  verification: { codesign: true, spctl_app: true, spctl_dmg: true, stapled_app: true, stapled_dmg: true }
}
```

Tests must fail independently for wrong identity/team/bundle/version, one architecture, no runtime flag, forbidden entitlement, rejected notarization, unstapled app/DMG, rejected Gatekeeper result, and checksum computed before final stapling.

- [ ] **Step 2: Run red**

```bash
node --test scripts/verify-macos-artifact.test.mjs
```

Expected: FAIL because the verifier module is absent.

- [ ] **Step 3: Implement command orchestration and parsers**

Export `verifyMacosArtifact(options, runner)` and invoke exactly:

```text
hdiutil attach -readonly -nobrowse <dmg>
codesign --verify --deep --strict --verbose=2 <mounted-app>
codesign -dv --verbose=4 <mounted-app>
codesign -d --entitlements :- <mounted-app>
spctl --assess --type execute --verbose=4 <mounted-app>
spctl --assess --type open --context context:primary-signature --verbose=4 <dmg>
xcrun stapler validate <mounted-app>
xcrun stapler validate <dmg>
lipo -archs <mounted-app>/Contents/MacOS/forge
shasum -a 256 <dmg>
hdiutil detach <mount-point>
```

Require `source=Notarized Developer ID`, both architectures, expected TeamIdentifier, `flags` containing `runtime`, and checksum after the two stapler validations. Always detach in `finally`.

- [ ] **Step 4: Run green and deliberate negative fixture**

```bash
node --test scripts/verify-macos-artifact.test.mjs
node scripts/verify-macos-artifact.mjs --help
```

Expected: tests PASS; help lists `--dmg`, `--tag`, `--commit-sha`, `--identity`, `--team-id`, `--notary-json`, and `--out`.

- [ ] **Step 5: Commit**

```bash
git add scripts/verify-macos-artifact.mjs scripts/verify-macos-artifact.test.mjs
git commit -m "feat(release): verify final notarized macOS bytes"
```

## Task 4: Define and validate clean-user installation evidence

**Files:**
- Create: `release/macos-install-smoke.schema.json`
- Create: `scripts/macos-install-smoke-preflight.mjs`
- Create: `scripts/macos-install-smoke-preflight.test.mjs`
- Create: `scripts/validate-macos-install-evidence.mjs`
- Create: `scripts/validate-macos-install-evidence.test.mjs`
- Create: `apps/desktop/docs/product/public-beta-install-smoke.md`

- [ ] **Step 1: Write failing schema/preflight/validator tests**

The schema requires `schema_version`, `tag`, `commit_sha`, `dmg_sha256`, `macos_version`, `architecture`, `clean_user`, `preexisting_forge_state`, `quarantine_present`, `gatekeeper_override_used`, `steps`, `session_id_before_quit`, `session_id_after_reopen`, `evidence_files`, `started_at`, and `completed_at`.

Required step ids are exactly:

```js
[
  "download",
  "checksum",
  "mount",
  "install",
  "first_launch",
  "permission_denied",
  "permission_approved",
  "bounded_provider_smoke",
  "quit",
  "reopen",
  "session_recovered",
  "cleanup"
]
```

Tests must reject `clean_user !== true`, any preexisting state, missing quarantine, any override, missing/failed/unknown step, different session ids, secret-like evidence content, missing evidence SHA, or DMG/tag/SHA mismatch.

- [ ] **Step 2: Run red**

```bash
node --test scripts/macos-install-smoke-preflight.test.mjs scripts/validate-macos-install-evidence.test.mjs
```

Expected: FAIL because files are absent.

- [ ] **Step 3: Implement schema, preflight, validator, and protocol**

The protocol must require a newly created standard macOS user, browser-originated download or explicit quarantine xattr, no `xattr -dr`, no right-click Open, no Security Settings override, a disposable repository, deny then approve permission, one bounded real-provider readiness/task smoke, normal quit, reopen, identical session id, and cleanup instructions. Evidence stores screenshot/log path, SHA-256, timestamp, and redaction status, never prompt/workspace body text.

- [ ] **Step 4: Run green**

```bash
node --test scripts/macos-install-smoke-preflight.test.mjs scripts/validate-macos-install-evidence.test.mjs
node scripts/macos-install-smoke-preflight.mjs --help
node scripts/validate-macos-install-evidence.mjs --help
```

Expected: PASS; both help commands document the fixed manifest/evidence relationship and exit 0.

- [ ] **Step 5: Commit**

```bash
git add release/macos-install-smoke.schema.json scripts/macos-install-smoke-preflight.mjs scripts/macos-install-smoke-preflight.test.mjs scripts/validate-macos-install-evidence.mjs scripts/validate-macos-install-evidence.test.mjs apps/desktop/docs/product/public-beta-install-smoke.md
git commit -m "feat(release): require clean-user macOS install evidence"
```

## Task 5: Finalize one deterministic R4 manifest

**Files:**
- Modify: `release/release-manifest.schema.json`
- Modify: `scripts/validate-release-manifest.mjs`
- Modify: `scripts/validate-release-manifest.test.mjs`
- Create: `scripts/finalize-public-release.mjs`
- Create: `scripts/finalize-public-release.test.mjs`

- [ ] **Step 1: Write failing R4 finalization tests**

Require the final manifest to preserve every R3 field byte-for-value and add:

```js
{
  release_state: "R4",
  artifacts: [{ kind: "macos_dmg", file_name, url, sha256, size_bytes, architectures: ["arm64", "x86_64"], minimum_system_version: "14.0" }],
  signing: { status: "verified", identity, team_id, hardened_runtime: true, entitlements_sha256, verification_evidence_sha256 },
  notarization: { status: "accepted", submission_id, log_sha256, stapled_app: true, stapled_dmg: true },
  installation_smoke: { status: "passed", clean_user: true, gatekeeper_override: false, session_recovery: true, evidence_url, evidence_sha256 },
  website: { channel: "public-beta", manifest_url, download_url, checksum_sha256 },
  previous_release: { tag, manifest_url, download_url, checksum_sha256 }
}
```

Reject missing previous release after the first beta, website/artifact URL drift, checksum drift, non-R3 input, mutable `latest` URLs, or generated time affecting deterministic content other than the explicit `generated_at` input.

- [ ] **Step 2: Run red**

```bash
node --test scripts/validate-release-manifest.test.mjs scripts/finalize-public-release.test.mjs
```

Expected: FAIL because R4 fields/finalizer are unsupported.

- [ ] **Step 3: Extend schema/validator and implement finalizer**

The finalizer CLI takes explicit `--candidate`, `--artifact-evidence`, `--install-evidence`, `--download-url`, `--manifest-url`, `--previous-manifest`, `--generated-at`, and `--out`. It must write with stable key ordering and refuse to overwrite an existing different output.

- [ ] **Step 4: Run green and determinism check**

```bash
node --test scripts/validate-release-manifest.test.mjs scripts/finalize-public-release.test.mjs
node scripts/finalize-public-release.mjs --help
```

Expected: PASS; identical inputs produce byte-identical manifests and changed evidence causes a nonzero overwrite refusal.

- [ ] **Step 5: Commit**

```bash
git add release/release-manifest.schema.json scripts/validate-release-manifest.mjs scripts/validate-release-manifest.test.mjs scripts/finalize-public-release.mjs scripts/finalize-public-release.test.mjs
git commit -m "feat(release): finalize immutable R4 manifests"
```

## Task 6: Replace the desktop release workflow with protected sign/notarize/publish stages

**Files:**
- Modify: `.github/workflows/desktop-release.yml`
- Create: `.github/workflows/macos-install-evidence.yml`
- Modify: `scripts/ci-workflow.test.mjs`

- [ ] **Step 1: Expand the failing workflow contract**

Assert that the workflows contain: exact `desktop-v*` trigger; `public-beta` environment; `persist-credentials: false`; exact R3 resolver; universal Rust targets; Apple certificate/API-key secrets; `--target universal-apple-darwin`; artifact verifier; install evidence validator; final manifest validator; `codesign`, `spctl`, `stapler`, checksum evidence; and `gh release create --verify-tag --prerelease`. Publishing must require explicit successful `staged_run_id` and `install_evidence_run_id` inputs whose head SHA equals the tag target, then download exact artifact names. Assert neither workflow contains `releases/latest`, created-at sorting, `|| true`, or secrets at workflow-global env.

- [ ] **Step 2: Run red**

```bash
node --test scripts/ci-workflow.test.mjs
```

Expected: FAIL on the existing upload-only workflow.

- [ ] **Step 3: Implement the staged workflow**

Use the staging/publishing jobs in this order:

```text
resolve-r3 -> build-sign-notarize -> upload-staged-release
macos-install-evidence(tag, staged_run_id) -> upload forge-macos-install-<tag>
publish-release(tag, staged_run_id, install_evidence_run_id)
```

`resolve-r3` checks tag ancestry/version and downloads exact `forge-r3-<tag-target-sha>` bytes. `build-sign-notarize` references environment `public-beta`, decodes certificate and API key only under `$RUNNER_TEMP`, masks derived sensitive values, installs both Rust targets, builds universal app/DMG, and uploads `forge-macos-staged-<tag>-<full-sha>`. `macos-install-evidence.yml` requires explicit `tag` and `staged_run_id`, runs on labels `[self-hosted, macOS, forge-clean-user]`, downloads only that staged artifact, reads operator evidence from `$HOME/ForgeReleaseEvidence/<tag>/macos-install-smoke.json`, validates it, and uploads `forge-macos-install-<tag>-<full-sha>`. The manual publish dispatch requires explicit `tag`, `staged_run_id`, and `install_evidence_run_id`; it verifies both runs succeeded for the tag SHA and downloads exact artifact names. `publish-release` has `contents: write`, revalidates every SHA, finalizes R4, creates a prerelease, uploads DMG/manifest/checksum/notary/install evidence, and refuses an existing tag whose assets differ.

Repository setup outside YAML is explicit: create `public-beta`, require a reviewer, prevent self-review, and add environment-only secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_P8_BASE64`, and `KEYCHAIN_PASSWORD`.

- [ ] **Step 4: Run workflow green contracts**

```bash
node --test scripts/ci-workflow.test.mjs
npm run check:ci
```

Expected: PASS; contract proves no publish path bypasses R3, artifact, or installation validation.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/desktop-release.yml .github/workflows/macos-install-evidence.yml scripts/ci-workflow.test.mjs
git commit -m "ci(release): sign notarize and gate public beta publication"
```

## Task 7: Add a cryptographically pinned website release loader

**Files:**
- Create: `apps/website/src/release.js`
- Create: `apps/website/src/release.test.mjs`
- Modify: `apps/website/package.json`
- Modify: `apps/website/package-lock.json`

- [ ] **Step 1: Write failing loader tests**

The module exports exactly three functions: asynchronous `sha256Hex(bytes)`, synchronous `validatePublicReleaseManifest(manifest)`, and asynchronous `loadPublicBetaRelease({ fetchImpl, channelUrl })`. `loadPublicBetaRelease` defaults `fetchImpl` to global `fetch` and `channelUrl` to `/releases/public-beta.json`.

Return `{ status: "ready", manifest, dmg }` only when the channel SHA matches fetched manifest bytes and all R4/signing/notarization/install/URL/checksum invariants pass. Return `{ status: "unavailable", reason }` for 404, invalid JSON, hash mismatch, R3, failed/unknown signing/notary/install, mutable URL, missing rollback, or website/artifact drift.

- [ ] **Step 2: Run red**

```bash
npm --prefix apps/website run test
```

Expected: FAIL because the test script and loader do not exist.

- [ ] **Step 3: Implement loader and package scripts**

Set scripts:

```json
{
  "test": "node --test src/release.test.mjs",
  "test:e2e": "playwright test"
}
```

Use Web Crypto SHA-256. Require versioned URLs containing `/releases/download/<manifest.version>/`; explicitly reject `/latest/` and query-selected artifacts.

- [ ] **Step 4: Run green**

```bash
npm --prefix apps/website run test
npm --prefix apps/website run build
```

Expected: loader tests PASS and production build exits 0 while no channel file exists; absence produces unavailable state, not a fake URL.

- [ ] **Step 5: Commit**

```bash
git add apps/website/src/release.js apps/website/src/release.test.mjs apps/website/package.json apps/website/package-lock.json
git commit -m "feat(website): load only verified beta manifests"
```

## Task 8: Replace fake CTAs and add SEO/accessibility/download smoke

**Files:**
- Modify: `apps/website/src/App.jsx`
- Modify: `apps/website/src/styles.css`
- Modify: `apps/website/index.html`
- Create: `apps/website/public/robots.txt`
- Create: `apps/website/playwright.config.mjs`
- Create: `apps/website/e2e/download.spec.mjs`
- Modify: `apps/website/package.json`
- Modify: `apps/website/package-lock.json`

- [ ] **Step 1: Write failing browser smoke**

Add dev dependencies `@playwright/test` and `@axe-core/playwright`. Tests route the channel and manifest requests with byte-consistent fixtures, then assert all three download links share the exact versioned URL, version/checksum/macOS 14+/privacy/security/rollback are visible, one h1 exists, title/description/canonical/OG metadata exist, keyboard focus is visible, and axe reports no WCAG A/AA violations. Invalid checksum and `installation_smoke.status = "unknown"` fixtures must expose no downloadable link.

- [ ] **Step 2: Run red**

```bash
npm --prefix apps/website run test:e2e
```

Expected: FAIL because Playwright config/spec are absent and current CTAs are buttons.

- [ ] **Step 3: Implement verified UI and metadata**

In `App`, load once with `useEffect`; remove `handleDownload`, download toast state, and all fake download buttons. Render `<a>` only for `status === "ready"`; otherwise render a disabled status reading `Public beta download is not yet verified.` Add one release details block with version, `Universal · macOS 14 or later`, full SHA-256, privacy/security link, release notes, and previous beta rollback link.

Set `lang="en"`, a relative canonical `/`, Open Graph title/description/image, `robots=index,follow`, and keep the real screenshot as the social image. Add `aria-live="polite"`, `aria-expanded`/`aria-controls` to FAQ buttons, and `:focus-visible` styles.

- [ ] **Step 4: Run green and real-link smoke contract**

```bash
npm --prefix apps/website run test
npm --prefix apps/website run build
npm --prefix apps/website run test:e2e
```

Expected: all pass; verified fixture links match exactly, invalid fixture has zero download links, axe has zero targeted violations.

- [ ] **Step 5: Commit**

```bash
git add apps/website/src/App.jsx apps/website/src/styles.css apps/website/index.html apps/website/public/robots.txt apps/website/playwright.config.mjs apps/website/e2e/download.spec.mjs apps/website/package.json apps/website/package-lock.json
git commit -m "feat(website): publish verified macOS beta download"
```

## Task 9: Wire the four fixed repository gates

**Files:**
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/ci-workflow.test.mjs`

- [ ] **Step 1: Write failing exact-label tests**

Require these label/command pairs:

```text
macOS signing configuration contract -> node --test scripts/macos-signing-config.test.mjs
public beta artifact verification contract -> node --test scripts/verify-macos-artifact.test.mjs scripts/finalize-public-release.test.mjs
public beta install evidence contract -> node --test scripts/macos-install-smoke-preflight.test.mjs scripts/validate-macos-install-evidence.test.mjs
website verified download contract -> npm --prefix apps/website run test && npm --prefix apps/website run build && npm --prefix apps/website run test:e2e
```

Mark the first three contract commands fast-contract except the live install run, which remains manual-evidence. Do not mark manual clean-user evidence as passed merely because its validator unit tests ran.

- [ ] **Step 2: Run red**

```bash
node --test scripts/acceptance.test.mjs scripts/ci-workflow.test.mjs
```

Expected: FAIL because the labels and CI paths are absent.

- [ ] **Step 3: Add gates and path coverage**

Add a `release` domain to acceptance domain rendering and CI path filters for `.github/workflows/desktop-release.yml`, `scripts/**`, `release/**`, `docs/public-beta.md`, and `apps/desktop/docs/product/public-beta-install-smoke.md`.

- [ ] **Step 4: Run green and advertised dry run**

```bash
node --test scripts/acceptance.test.mjs scripts/ci-workflow.test.mjs
scripts/acceptance.sh --dry-run --grep "public beta"
scripts/acceptance.sh --dry-run --only "macOS signing configuration contract"
scripts/acceptance.sh --dry-run --only "website verified download contract"
```

Expected: tests PASS; dry runs print the exact fixed labels/commands without executing live release actions.

- [ ] **Step 5: Commit**

```bash
git add scripts/acceptance.sh scripts/acceptance.test.mjs .github/workflows/ci.yml scripts/ci-workflow.test.mjs
git commit -m "test(release): register public beta distribution gates"
```

## Task 10: Publish user-facing system, privacy, security, checksum, and rollback documentation

**Files:**
- Create: `docs/public-beta.md`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add a failing documentation contract to the existing fixed-label tests**

Assert the docs contain: macOS 14+, Universal arm64/x86_64, Developer ID, notarization/stapling, checksum command, no Gatekeeper bypass, provider credentials stored locally, no prompt/workspace telemetry claim, permission deny/approve behavior, uninstall paths, and previous-beta rollback.

- [ ] **Step 2: Run red**

```bash
node --test scripts/acceptance.test.mjs
```

Expected: FAIL on missing `docs/public-beta.md` and release documentation phrases.

- [ ] **Step 3: Write the public-beta documentation**

Document `shasum -a 256 Forge_0.1.0-beta.1_universal.dmg`, macOS 14 or later, both architectures, drag-to-Applications install, normal Gatekeeper first launch, `~/Library/Application Support`/`~/.forge` cleanup boundaries, local provider key handling, permission prompts, redacted diagnostics, known beta limitations, and the immutable previous-release link sourced from the manifest. Do not claim automatic updates or Mac App Store distribution.

- [ ] **Step 4: Run green**

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Expected: PASS; documentation never claims R4 independently of machine-readable evidence.

- [ ] **Step 5: Commit**

```bash
git add docs/public-beta.md README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.test.mjs
git commit -m "docs(release): publish beta install and rollback guidance"
```

## Task 11: Execute the first live release and promote the website channel

**Files:**
- Create: `apps/website/public/releases/manifests/desktop-v0.1.0-beta.1.json`
- Create: `apps/website/public/releases/public-beta.json`

- [ ] **Step 1: Prove the protected environment is ready without exposing values**

Run:

```bash
gh secret list --repo Cabbos/forge --env public-beta
gh api repos/Cabbos/forge/environments/public-beta
```

Expected: all seven required secret names exist, at least one required reviewer exists, and self-review prevention is enabled. Secret values are never printed.

- [ ] **Step 2: Create and push the annotated immutable tag**

```bash
git fetch origin main --tags
git merge-base --is-ancestor HEAD origin/main
git tag -a desktop-v0.1.0-beta.1 -m "Forge desktop v0.1.0-beta.1"
git push origin desktop-v0.1.0-beta.1
```

Expected: HEAD is on `main` lineage and the remote accepts a new tag; an existing different tag is a hard stop.

- [ ] **Step 3: Run the release workflow through R3 and final artifact verification**

```bash
gh workflow run desktop-release.yml --ref desktop-v0.1.0-beta.1 -f mode=stage -f tag=desktop-v0.1.0-beta.1
gh run watch "$STAGED_RUN_ID" --exit-status
```

Expected before manual evidence: exact R3 artifact resolved; universal signed/notarized/stapled DMG passes `codesign`, `spctl`, stapler, architecture, and checksum checks; workflow pauses at the protected install-evidence boundary and the website remains unchanged.

- [ ] **Step 4: Run and upload clean-user evidence**

On the clean account, follow `apps/desktop/docs/product/public-beta-install-smoke.md`, then run:

```bash
node scripts/validate-macos-install-evidence.mjs \
  --manifest release/evidence/desktop-v0.1.0-beta.1/candidate-manifest.json \
  --artifact-evidence release/evidence/desktop-v0.1.0-beta.1/macos-artifact.json \
  --evidence release/evidence/desktop-v0.1.0-beta.1/macos-install-smoke.json
gh workflow run macos-install-evidence.yml \
  --ref desktop-v0.1.0-beta.1 \
  -f tag=desktop-v0.1.0-beta.1 \
  -f staged_run_id="$STAGED_RUN_ID"
```

Set `STAGED_RUN_ID` from the explicit run URL returned in the Actions UI; do not derive it by taking the newest run. The approved clean-user runner writes `$HOME/ForgeReleaseEvidence/desktop-v0.1.0-beta.1/macos-install-smoke.json` while the evidence workflow is waiting, after which the workflow validates and uploads exact artifact `forge-macos-install-desktop-v0.1.0-beta.1-<full-sha>`. Expected: validator reports `status: passed`, no override, identical session ids, and a successful evidence run bound to the staged run id.

- [ ] **Step 5: Verify published immutable bytes**

```bash
gh release view desktop-v0.1.0-beta.1 --json tagName,isPrerelease,assets,url
curl -fL https://github.com/Cabbos/forge/releases/download/desktop-v0.1.0-beta.1/release-manifest.json -o /tmp/forge-release-manifest.json
node scripts/validate-release-manifest.mjs --manifest /tmp/forge-release-manifest.json --require-state R4
curl -fL https://github.com/Cabbos/forge/releases/download/desktop-v0.1.0-beta.1/Forge_0.1.0-beta.1_universal.dmg -o /tmp/Forge_0.1.0-beta.1_universal.dmg
shasum -a 256 /tmp/Forge_0.1.0-beta.1_universal.dmg
```

Expected: release is prerelease, manifest is R4, downloaded SHA equals manifest, and the previous beta remains listed once a second beta exists. Enable GitHub immutable releases before considering this evidence complete; if the repository cannot enable it, use versioned write-once storage and record that URL in the manifest.

- [ ] **Step 6: Promote the exact manifest snapshot and verify the website**

Copy the downloaded manifest byte-for-byte to `apps/website/public/releases/manifests/desktop-v0.1.0-beta.1.json`. Generate `public-beta.json` with `schema_version: 1`, `channel: "public-beta"`, `manifest_path: "/releases/manifests/desktop-v0.1.0-beta.1.json"`, and the computed manifest SHA-256. Then run:

```bash
npm --prefix apps/website run test
npm --prefix apps/website run build
npm --prefix apps/website run test:e2e
```

Expected: CTA resolves only to the versioned DMG; checksum, macOS 14+, privacy/security, and rollback information match the manifest.

- [ ] **Step 7: Commit the channel promotion**

```bash
git add apps/website/public/releases/manifests/desktop-v0.1.0-beta.1.json apps/website/public/releases/public-beta.json
git commit -m "release(website): promote desktop v0.1.0 beta 1"
```

- [ ] **Step 8: Exercise rollback without rebuilding old bytes**

Change only `apps/website/public/releases/public-beta.json` to the prior manifest path/SHA recorded in `previous_release`, run website test/build/e2e, and verify the old versioned DMG checksum. Revert the exercise before merging the first-release promotion because no prior public beta exists yet. For beta 2 and later, retain the rollback commit as the production recovery operation.

## Final verification

Run from a clean checkout after all code tasks and before creating the live tag:

```bash
node --test \
  scripts/resolve-r3-candidate.test.mjs \
  scripts/release-version.test.mjs \
  scripts/macos-signing-config.test.mjs \
  scripts/verify-macos-artifact.test.mjs \
  scripts/macos-install-smoke-preflight.test.mjs \
  scripts/validate-macos-install-evidence.test.mjs \
  scripts/validate-release-manifest.test.mjs \
  scripts/finalize-public-release.test.mjs \
  scripts/ci-workflow.test.mjs \
  scripts/acceptance.test.mjs
npm run build:desktop
npm run build:website
npm --prefix apps/website run test
npm --prefix apps/website run test:e2e
scripts/acceptance.sh --dry-run
```

Expected: all contract tests and builds pass; dry-run advertises all four fixed labels; no live release or website channel mutation occurs during verification.

Before each implementation edit, refresh GitNexus or attach the required fallback impact report. Minimum impact targets are `App` in `apps/website/src/App.jsx`, `evaluateRestartHarness` if the existing preflight is touched, `startup_restore_active_session` in `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`, `flush_all_sessions` in `apps/desktop/src-tauri/src/autosave.rs`, and `run` in `apps/desktop/src-tauri/src/lib.rs`. Before every commit, run `detect_changes({scope: "compare", base_ref: "main"})` and confirm only the intended release, website, evidence, and documentation authority domains changed.
