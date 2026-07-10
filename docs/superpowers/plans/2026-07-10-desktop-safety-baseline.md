# Desktop Safety Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Desktop R1 public-beta safety blockers by making repository execution fail closed, moving provider secrets to macOS Keychain, redacting sensitive persistent logs centrally, restoring Git worktrees without data loss, hardening Tauri CSP/capabilities, and clearing the deterministic red signals that would otherwise hide regressions.

**Architecture:** Keep the Rust backend authoritative. Shell classification produces explicit semantic decisions consumed by the permission gate; a narrow injected credential-store interface owns secret values; every persistent-log path passes through one redactor; checkpoint restore uses a versioned complete Git snapshot and a pre-mutation rollback snapshot; and Tauri grants only the browser capabilities used by the frontend. Every slice lands with focused regression tests and one of five exact release-required acceptance labels.

**Tech Stack:** Rust 2021, Tauri 2, macOS Keychain through the Rust `keyring` crate, serde/serde_json, Git CLI plumbing, React 18, TypeScript, Tailwind CSS 4, Vite 8, Node test runner, Playwright, Bash acceptance matrix, GitNexus impact and change detection.

---

## Source Of Truth And Scope

This plan implements subproject **A. Desktop Safety Baseline** and the **R1: Safety Baseline Green** evidence in `docs/superpowers/specs/2026-07-10-public-beta-convergence-design.md`.

R1 is green only when all of these are true on the same commit:

- Project-defined package scripts, test runners, compilers, Rust build hooks, and equivalent commands are never classified as read-only inspection.
- Unknown shell commands require an explicit user decision in manual, trusted-project, and full-access modes.
- Catastrophic commands and external-workspace writes remain blocked in every permission mode.
- Provider secrets are stored and resolved through a system credential-store abstraction; ordinary Forge JSON contains references and status only.
- Persistent logs redact credentials, authorization headers, request bodies, environment values, and hidden context before bytes reach disk.
- Checkpoint restore round-trips staged, unstaged, untracked, renamed, deleted, and binary state, or refuses an unsupported state before mutation.
- Desktop CSP is non-null, Tauri capabilities are least-privilege, deterministic backend/build/browser signals are clean, and the complete backend gate passes.

Out of scope:

- A general-purpose command sandbox.
- New providers, runtime owners, or permission modes.
- Cloud secret synchronization.
- Replacing Git with an in-process Git implementation.
- Signing, notarization, release-manifest publication, and website distribution; those consume these gates in later subprojects.
- Extracting a shared package from the desktop app.

## Frozen Planning Evidence

The 2026-07-10 baseline has three deterministic signals that must be repaired before security changes:

1. `ipc::handlers::tests::forgotten_memory_not_injected_via_select_context` expects legacy memory ID `will-forget`; `UnifiedMemoryRecord::from_wiki_memory` now emits `wiki_memory:will-forget`.
2. Vite completes with 66 LightningCSS unknown-at-rule warnings because Tailwind 3 PostCSS processes Tailwind 4 `tw-animate-css` and `shadcn/tailwind.css` inputs. The component set already uses Tailwind 4 variants and shorthand, so removing those imports would silently remove UI behavior; this plan migrates the pipeline to Tailwind 4.
3. The Playwright IPC fixture has no `list_continuity_experiences`, `search_continuity_experiences`, or `update_continuity_experience_status` cases, so its default branch returns `undefined` to React Query.

The GitNexus index was stale during planning. Before each source-symbol edit below, the implementer must run the named `impact(...)` calls. If any call reports HIGH or CRITICAL risk, report its direct callers, affected processes, and risk to the user before editing. If GitNexus is stale, refresh it from the repository root:

```bash
node scripts/gitnexus-safe.mjs -- pnpm --allow-build=@ladybugdb/core --allow-build=gitnexus --allow-build=tree-sitter --allow-build=tree-sitter-kotlin dlx gitnexus@latest analyze --index-only
```

If refresh or impact times out, record the required fallback impact report in the implementation task before editing: attempted command, exact error, index freshness, symbols searched, files inspected, direct callers found manually, selected tests, affected authority domains, and residual risk. Before every commit, run:

```text
detect_changes({ repo: "forge", scope: "compare", base_ref: "main" })
```

Confirm the changed symbols and execution flows match that task. If change detection is unavailable, record the same fallback fields plus `git diff --stat main...HEAD` and `git diff --name-only main...HEAD`.

## Release-Required Acceptance Labels

The label strings are an external contract. Add them verbatim to `scripts/acceptance.sh` and its exact ordered contract in `scripts/acceptance.test.mjs`; do not rename, title-case, or combine them.

| Required label | Evidence owned by this plan |
|---|---|
| `desktop deterministic signal cleanup` | Unified-memory assertion, warning-free production CSS build, console-clean continuity query fixture |
| `desktop command execution safety baseline` | Shell classifier, permission matrix, malicious package/build-hook/test fixtures, complete executor preflight |
| `desktop credential and redaction safety baseline` | Keychain abstraction, plaintext migration, unavailable-store fail-closed behavior, central persistent-log redaction |
| `desktop checkpoint restore safety baseline` | Complete staged/unstaged/untracked/rename/delete/binary capture, preflight refusal, rollback proof |
| `desktop CSP and capability safety baseline` | Non-null production/dev CSP, minimal Tauri capabilities, frontend capability-use contract, no-bundle packaged build |

Place all five under `set_domain 'desktop-safety'`. After the fifth gate, restore the domain expected by the next existing gate. Each command must be unique because the acceptance contract rejects duplicate commands.

## Target File Structure

New focused Rust modules:

- `apps/desktop/src-tauri/src/credential_store.rs` — `CredentialRef`, `CredentialStore`, macOS Keychain implementation, unavailable implementation, and in-memory test double.
- `apps/desktop/src-tauri/src/credential_migration.rs` — idempotent migration of legacy settings/profile plaintext into references after verified Keychain writes.
- `apps/desktop/src-tauri/src/redaction.rs` — the only persistent text/JSON redaction policy and secret-value registry.
- `apps/desktop/src-tauri/src/ipc/checkpoint_snapshot.rs` — versioned complete Git snapshot capture, validation, materialization, and equality helpers.

New focused JavaScript contracts:

- `apps/desktop/scripts/desktop-deterministic-signals.test.mjs` — runs the real production build and rejects CSS warning regressions.
- `apps/desktop/scripts/desktop-security-config.test.mjs` — parses Tauri configuration/capabilities and scans frontend imports against the allowlist.

Existing authority modules remain the integration points:

- `harness/shell_policy.rs` classifies commands; `harness/permissions.rs` makes the permission decision; `harness/mod.rs` remains the sole pre-execution gate.
- `settings.rs`, `profile/mod.rs`, `state.rs`, and settings/session/provider handlers resolve secret references without serializing values.
- `logger.rs` and `log_store.rs` enforce redaction before every persistent write.
- `ipc/project_checkpoint.rs` orchestrates checkpoint IPC using `ipc/checkpoint_snapshot.rs` primitives.
- `tauri.conf.json` and `capabilities/default.json` declare browser authority.

---

## Task Group 1: Deterministic Signal Cleanup

### Task 1: Repair The Unified-Memory Assertion And Make The CSS Build Warning-Free

**Files:**

- Modify: `apps/desktop/src-tauri/src/ipc/handlers_tests.rs`
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/package-lock.json`
- Modify: `apps/desktop/postcss.config.js`
- Rename: `apps/desktop/tailwind.config.ts` to `apps/desktop/tailwind.config.js`
- Modify: `apps/desktop/src/styles/globals.css`
- Create: `apps/desktop/scripts/desktop-deterministic-signals.test.mjs`

- [x] **Step 1: Run impact analysis before touching the asserted data contract or build entry point**

Run:

```text
impact({ repo: "forge", target: "forgotten_memory_not_injected_via_select_context", direction: "upstream" })
impact({ repo: "forge", target: "UnifiedMemoryRecord::from_wiki_memory", direction: "upstream" })
impact({ repo: "forge", target: "unified_selection_to_selected_context", direction: "upstream" })
```

Expected blast radius: the first symbol is test-only; the production symbols feed unified-memory selection into `send_input`. This task changes only the stale expectation, not production ID construction. Report any HIGH or CRITICAL result before proceeding.

- [x] **Step 2 (Red): Reproduce the stale Rust assertion**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::handlers::tests::forgotten_memory_not_injected_via_select_context --lib -- --exact
```

Expected: one failure showing that the selected ID is source-qualified instead of equal to `will-forget`.

- [x] **Step 3 (Green): Update the assertion to the canonical unified ID**

In `forgotten_memory_not_injected_via_select_context`, change only the positive-selection expectation:

```rust
assert!(selected
    .memories
    .iter()
    .any(|memory| memory.memory_id == "wiki_memory:will-forget"));
```

Keep the forgotten-memory negative assertion intact so the test still proves that `wiki_memory:will-forget` is omitted after forgetting.

Run the focused command again. Expected: one passed, zero failed.

- [ ] **Step 4 (Red): Add a production-build warning contract**

Create `apps/desktop/scripts/desktop-deterministic-signals.test.mjs` with a test that runs the real build from `apps/desktop` and includes complete output on failure:

```js
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

const desktopDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("desktop production build has no unresolved Tailwind CSS at-rules", () => {
  const result = spawnSync("npm", ["run", "build"], {
    cwd: desktopDir,
    encoding: "utf8",
    env: { ...process.env, NO_COLOR: "1" },
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;

  assert.equal(result.status, 0, output);
  assert.doesNotMatch(output, /Unknown at rule/i);
  assert.doesNotMatch(output, /\b(?:@theme|@utility|@custom-variant)\b.*warning/i);
});
```

Add this package script:

```json
"check:deterministic-signals": "node --test scripts/desktop-deterministic-signals.test.mjs"
```

Run:

```bash
npm --prefix apps/desktop run check:deterministic-signals
```

Expected: the build itself succeeds, but the test fails because the captured output contains the current unknown-at-rule warnings.

- [ ] **Step 5 (Green): Move the desktop pipeline coherently to Tailwind 4**

From `apps/desktop`, update and lock the build dependencies:

```bash
npm install --save-dev tailwindcss@^4 @tailwindcss/postcss@^4
npm uninstall --save-dev autoprefixer
```

Expected: `package.json` and `package-lock.json` use the Tailwind 4 and `@tailwindcss/postcss` major versions, and the direct `autoprefixer` dependency is gone.

Replace `postcss.config.js` with:

```js
export default {
  plugins: {
    "@tailwindcss/postcss": {},
  },
};
```

Rename `tailwind.config.ts` to `tailwind.config.js`, remove the TypeScript-only import and `satisfies Config`, and preserve the existing `theme.extend` values exactly:

```js
export default {
  content: [],
  theme: {
    extend: {
      // Preserve every existing color and borderRadius entry verbatim.
    },
  },
  plugins: [],
};
```

Use `git diff --no-index apps/desktop/tailwind.config.ts apps/desktop/tailwind.config.js` before deleting the old file to verify the only semantic changes are `content`, the type import, and `satisfies Config`.

Replace the Tailwind header in `src/styles/globals.css` with this ordered header; keep all Forge stylesheet imports and existing component rules after it:

```css
@import "tailwindcss";
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@fontsource-variable/geist";

@config "../../tailwind.config.js";
@source "../../index.html";
@source "../**/*.{js,ts,jsx,tsx}";
```

Remove the Tailwind 3 directives:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

Do not remove `tw-animate-css` or `shadcn/tailwind.css`; `dialog.tsx`, `dropdown-menu.tsx`, and `tooltip.tsx` consume their Tailwind 4 variants.

- [ ] **Step 6 (Refactor): Verify generated utilities and visual primitives, not only warning text**

Run:

```bash
npm --prefix apps/desktop run check:deterministic-signals
rg -n "animate-in|fade-in|zoom-in|slide-in-from" apps/desktop/dist/assets/*.css
rg -n "forge-app-shell|bg-popover|text-muted-foreground" apps/desktop/dist/assets/*.css
```

Expected: the Node test passes with zero unknown-at-rule warnings, and both `rg` commands find generated CSS. If the production build emits any new warning, treat it as a failing deterministic signal instead of weakening the assertion.

- [ ] **Step 7: Run change detection and commit the first green slice**

Run `detect_changes({ repo: "forge", scope: "compare", base_ref: "main" })`. Expected scope: one Rust test and frontend build configuration only.

```bash
git add apps/desktop/src-tauri/src/ipc/handlers_tests.rs apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/postcss.config.js apps/desktop/tailwind.config.js apps/desktop/src/styles/globals.css apps/desktop/scripts/desktop-deterministic-signals.test.mjs
git commit -m "fix(desktop): clear deterministic backend and css signals"
```

Expected: one commit; no generated `dist` files staged.

### Task 2: Make The Continuity Fixture Contract-Shaped And Register The First Acceptance Gate

**Files:**

- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [x] **Step 1: Run impact analysis for the queried frontend contract**

Run:

```text
impact({ repo: "forge", target: "useContinuityExperiencesQuery", direction: "upstream" })
impact({ repo: "forge", target: "listContinuityExperiences", direction: "upstream" })
impact({ repo: "forge", target: "ContinuityExperiencesSection", direction: "upstream" })
```

Expected: the hook feeds the Project Archive continuity surface. No production hook or IPC wrapper should change; the defect is the browser fixture returning `undefined`.

- [x] **Step 2 (Red): Add a console-clean Project Archive acceptance test**

Add a test named exactly `continuity query stays console-clean` to `apps/desktop/e2e/acceptance.spec.ts`. Reuse the file's existing project-open helper/setup and record console errors before opening Project Archive:

```ts
test("continuity query stays console-clean", async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });

  await page.getByRole("button", { name: "打开项目档案" }).click();
  await expect(page.getByText("经验回忆")).toBeVisible();
  await expect(page.getByText("还没有经验")).toBeVisible();

  expect(consoleErrors).not.toContainEqual(
    expect.stringContaining("Query data cannot be undefined"),
  );
});
```

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts --grep "continuity query stays console-clean"
```

Expected: failure because the console contains React Query's undefined-data error or the empty state never settles.

- [x] **Step 3 (Green): Give all three continuity commands typed fixture responses**

In `apps/desktop/e2e/fixtures/app.ts`, add a mutable fixture-local array with the same fields as `ContinuityExperience`:

```ts
let continuityExperiences: Array<{
  id: string;
  kind: "lesson" | "bug_pattern" | "workflow" | "decision" | "preference" | "project_fact";
  status: "candidate" | "accepted" | "pinned" | "forgotten" | "archived";
  title: string;
  body: string;
  project_path: string | null;
  source_session_id: string | null;
  confidence: number;
  created_at_ms: number;
  updated_at_ms: number;
  tags: string[];
}> = [];
```

Add cases before the fixture switch's `default` branch:

```ts
case "list_continuity_experiences":
  return continuityExperiences.filter((experience) =>
    !args.workingDir || experience.project_path === args.workingDir,
  );
case "search_continuity_experiences": {
  const query = String(args.query ?? "").trim().toLocaleLowerCase();
  const matches = continuityExperiences.filter((experience) =>
    (!args.workingDir || experience.project_path === args.workingDir) &&
    `${experience.title}\n${experience.body}\n${experience.tags.join(" ")}`
      .toLocaleLowerCase()
      .includes(query),
  );
  return matches.slice(0, Number(args.limit ?? matches.length));
}
case "update_continuity_experience_status": {
  const index = continuityExperiences.findIndex(
    (experience) => experience.id === args.experienceId,
  );
  if (index < 0) throw new Error("continuity experience not found");
  continuityExperiences[index] = {
    ...continuityExperiences[index],
    status: args.status,
    updated_at_ms: Date.now(),
  };
  return continuityExperiences[index];
}
```

Use the fixture's actual normalized argument type if its local name differs, but preserve these wire keys exactly: `workingDir`, `query`, `limit`, `experienceId`, and `status`.

Run the focused Playwright command again. Expected: one passed test and no React Query error.

- [x] **Step 4 (Green): Add the deterministic acceptance label and ordered contract entry**

Under a new `set_domain 'desktop-safety'` block in `scripts/acceptance.sh`, add:

```bash
add_gate 'desktop deterministic signal cleanup' 'npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts --grep "continuity query stays console-clean"'
```

Add the same label/command at the same relative position in `expectedEntries` in `scripts/acceptance.test.mjs`. Preserve the surrounding gate order.

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run | rg -F "desktop deterministic signal cleanup"
scripts/acceptance.sh --only "desktop deterministic signal cleanup"
```

Expected: the contract test passes, dry-run prints the label once, and the selected gate passes.

- [x] **Step 5 (Refactor): Run the focused mocked product smoke and inspect browser errors**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
```

Expected: all acceptance scenarios pass and the run contains no `Query data cannot be undefined` error.

- [x] **Step 6: Run change detection and commit**

Run GitNexus change detection. Expected scope: Playwright fixture, one product smoke, and acceptance metadata only.

```bash
git add apps/desktop/e2e/fixtures/app.ts apps/desktop/e2e/acceptance.spec.ts scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "test(desktop): make deterministic safety signals release-required"
```

---

## Task Group 2: Command Execution Safety

### Task 3: Replace The Read-Only Shell Bucket With Explicit Fail-Closed Classes

**Files:**

- Modify: `apps/desktop/src-tauri/src/harness/shell_policy.rs`
- Modify: `apps/desktop/src-tauri/src/harness/permissions.rs`
- Modify: `apps/desktop/src-tauri/src/harness/permission_ledger.rs`
- Modify: `apps/desktop/src-tauri/src/executor/mod.rs`

- [ ] **Step 1: Run mandatory impact analysis and report the blast radius**

Run:

```text
impact({ repo: "forge", target: "classify_shell_command", direction: "upstream" })
impact({ repo: "forge", target: "validate_shell_command_failsafe", direction: "upstream" })
impact({ repo: "forge", target: "PermissionGate::check_with_evidence", direction: "upstream" })
impact({ repo: "forge", target: "ToolExecutor::execute", direction: "upstream" })
```

Expected direct flow: `classify_shell_command` feeds `PermissionGate::check_with_evidence`; `validate_shell_command_failsafe` is the executor's defense in depth; `Harness::execute_tool_with_emitter` invokes the gate before executor dispatch. This authority boundary may report HIGH or CRITICAL risk. If it does, warn the user with the affected processes before editing and keep the change isolated to this task.

- [ ] **Step 2 (Red): Replace permissive unit expectations with the R1 matrix**

In `harness/shell_policy.rs`, add failing table-driven tests covering these exact outcomes:

| Command | Expected outcome |
|---|---|
| `git status --short` | allow inspection |
| `git diff -- README.md` | allow inspection |
| `rg -n credential src` | allow inspection |
| `npm test` | explicit confirmation, `ProjectDefinedExecution` |
| `npm run build` | explicit confirmation, `ProjectDefinedExecution` |
| `pnpm test` | explicit confirmation, `ProjectDefinedExecution` |
| `cargo test` | explicit confirmation, `ProjectDefinedExecution` |
| `cargo check` | explicit confirmation, `ProjectDefinedExecution` |
| `cargo build` | explicit confirmation, `ProjectDefinedExecution` |
| `cargo run` | explicit confirmation, `ProjectDefinedExecution` |
| `make test` | explicit confirmation, `ProjectDefinedExecution` |
| `python -m pytest` | explicit confirmation, `ProjectDefinedExecution` |
| `./scripts/verify.sh` | explicit confirmation, `ProjectDefinedExecution` |
| `some-new-tool inspect` | explicit confirmation, `UnknownExecution` |
| `cat /etc/hosts` | explicit confirmation, `ExternalRead` |
| `ls /etc` | explicit confirmation, `ExternalRead` |
| `rg -n credential ../outside` | explicit confirmation, `ExternalRead` |
| `cp README.md /tmp/forge-copy` | blocked, `ExternalWrite` |
| `printf x > ../outside.txt` | blocked, `ExternalWrite` |
| `rm -rf /` | blocked, `Catastrophic` |
| `git clean -fdx` | blocked, `Catastrophic` |

Delete or invert the existing tests that expect `npm test`, `npm run build`, `cargo test`, and `cargo check` to be `AllowReadonly`.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
```

Expected: failures for project-defined execution and external-write cases under the old enum and prefix list.

- [ ] **Step 3 (Green): Introduce one semantic decision model**

Before the inspection allowlist is consulted, canonicalize every path argument and classify any path outside the approved workspace as `ExternalRead` or `ExternalWrite`. Thus `ls /etc` and `rg credential ../outside` cannot be rescued by the `ls`/`rg` inspection allowlist. Add regression tests for both commands and for symlinked paths that escape the workspace.

Replace the current shell decision and safety types with these serializable, comparable types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPolicyDecision {
    AllowInspection,
    RequireExplicitConfirmation {
        risk: ShellRisk,
        reason: ShellPolicyReason,
    },
    Block {
        reason: ShellPolicyReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellRisk {
    Normal,
    Dangerous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPolicyReason {
    ProvenInspection,
    ProjectDefinedExecution,
    UnknownExecution,
    ExternalRead,
    ExternalWrite,
    ShellControl,
    DangerousMutation,
    Catastrophic,
}
```

Add `ShellPolicyReason::as_str()` with stable snake-case values for evidence:

```rust
pub const fn as_str(self) -> &'static str {
    match self {
        Self::ProvenInspection => "proven_inspection",
        Self::ProjectDefinedExecution => "project_defined_execution",
        Self::UnknownExecution => "unknown_execution",
        Self::ExternalRead => "external_read",
        Self::ExternalWrite => "external_write",
        Self::ShellControl => "shell_control",
        Self::DangerousMutation => "dangerous_mutation",
        Self::Catastrophic => "catastrophic",
    }
}
```

Refactor `classify_shell_command` into an ordered fail-closed classifier:

1. Empty input is `Block { Catastrophic }` with a validation error.
2. Catastrophic patterns are `Block { Catastrophic }`.
3. An external path combined with write-capable syntax is `Block { ExternalWrite }`.
4. Shell control that can hide a second command (`;`, `&&`, `||`, command substitution, backticks, newline, process substitution, `eval`, `sh -c`, `bash -c`) is `RequireExplicitConfirmation { Dangerous, ShellControl }` unless a stronger block already matched.
5. A narrow allowlist of inspection commands is `AllowInspection`.
6. Known package managers, test runners, compilers, build systems, local executable paths, and scripts are `RequireExplicitConfirmation { Normal, ProjectDefinedExecution }`.
7. External-path reads are `RequireExplicitConfirmation { Normal, ExternalRead }`.
8. Known workspace mutations are `RequireExplicitConfirmation { Normal or Dangerous, DangerousMutation }`.
9. Everything else is `RequireExplicitConfirmation { Dangerous, UnknownExecution }`.

The proven-inspection allowlist may include bounded `git status`, `git diff`, `git log`, `git show`, `rg`, `grep`, `ls`, `pwd`, `wc`, and read-only localhost health probes. It must not include any package-manager, compiler, interpreter, test runner, `find -exec`, or executable project path.

Implement `command_may_write` and `references_external_path` conservatively. Treat redirects (`>`, `>>`), output flags, copy/move/remove/install commands, `tee`, `sed -i`, `find -delete`, archive extraction, and mutating Git subcommands as write-capable. An external path is absolute, home-relative, parent-relative, or a resolved path outside the provided workspace root. Pass the normalized workspace root into classification; do not infer safety from string prefixes alone.

- [ ] **Step 4 (Green): Make the executor failsafe reject every non-approved class**

Change `validate_shell_command_failsafe` to return a structured error for `Block` and to accept `AllowInspection` or a command carrying a permission approval token issued by the gate. Do not let the executor reclassify `RequireExplicitConfirmation` as safe by itself.

Add an unforgeable per-call approval marker to the internal tool execution context, for example:

```rust
pub(crate) struct ShellApproval {
    command_digest: [u8; 32],
    reason: ShellPolicyReason,
}
```

`PermissionGate` creates it only after the user accepts the exact normalized command; `ToolExecutor` verifies the digest matches the command it will spawn. Keep this type Rust-internal and never deserialize it from frontend tool input.

If adding an approval marker would require widening a CRITICAL execution-flow signature beyond this task, keep the existing gate/executor call shape and make `validate_shell_command_failsafe` enforce hard blocks only; record the residual time-of-check/time-of-use risk explicitly and schedule the marker before declaring this gate green. R1 cannot be marked green with an unbound frontend approval.

- [ ] **Step 5 (Refactor): Map semantic reasons into stable ledger evidence**

Update `permission_ledger.rs` so shell events record:

- `AutomaticAllow` + `Safe` only for `ProvenInspection`.
- `ManualRequired` + `Caution` for `ProjectDefinedExecution` and `ExternalRead`.
- `ManualRequired` + `High` for `UnknownExecution`, `ShellControl`, and dangerous workspace mutation.
- `BlockedExternalPath` + `High` for `ExternalWrite`.
- `BlockedPolicy` + `High` for `Catastrophic`.

Put `ShellPolicyReason::as_str()` in the ledger reason field. Never include the raw command in the reason; existing bounded command summaries may remain in non-secret transient UI events.

- [ ] **Step 6: Run focused policy tests**

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_ledger --lib
```

Expected: all focused tests pass; no test retains the `AllowReadonly` name or expects project execution to auto-run.

- [ ] **Step 7: Run change detection and commit the classifier slice**

Expected affected flow: shell input → classification → permission evidence → executor failsafe. No frontend decision code should become authoritative.

```bash
git add apps/desktop/src-tauri/src/harness/shell_policy.rs apps/desktop/src-tauri/src/harness/permissions.rs apps/desktop/src-tauri/src/harness/permission_ledger.rs apps/desktop/src-tauri/src/executor/mod.rs
git commit -m "fix(desktop): classify project execution as explicit risk"
```

### Task 4: Enforce The Permission-Mode Matrix With Malicious Repository Fixtures

**Files:**

- Modify: `apps/desktop/src-tauri/src/harness/permissions_test.rs`
- Modify: `apps/desktop/src-tauri/tests/harness_test.rs`
- Modify: `apps/desktop/src-tauri/src/harness/mod.rs`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Run impact analysis for the authoritative execution path**

Run:

```text
impact({ repo: "forge", target: "Harness::execute_tool_with_emitter", direction: "upstream" })
impact({ repo: "forge", target: "PermissionGate::check_with_evidence", direction: "upstream" })
impact({ repo: "forge", target: "PermissionMode", direction: "upstream" })
```

Expected: session and headless agent tool execution flows depend on this path. Warn before editing if risk is HIGH or CRITICAL.

- [ ] **Step 2 (Red): Add a complete mode-by-command permission matrix**

In `harness/permissions_test.rs`, use the existing temporary-project helpers and assert these outcomes for `Manual`, `TrustProject`, and `FullAccess`:

| Class | Manual | TrustProject | FullAccess |
|---|---|---|---|
| proven inspection | automatic allow | automatic allow | automatic allow |
| project-defined execution | ask user | ask user | ask user |
| unknown execution | ask user | ask user | ask user |
| external read | ask user | ask user | ask user |
| external write | blocked | blocked | blocked |
| catastrophic | blocked | blocked | blocked |

The full-access expectations are intentional: R1 says unknown risk requires an explicit user decision and project scripts must not silently execute. Full access can continue to affect ordinary typed file tools; it cannot bypass these shell classes.

Add ledger assertions for decision kind, risk tier, and reason string for every row.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
```

Expected: old full-access and read-only behavior fails at least the project-defined, unknown, external-write, and catastrophic rows.

- [ ] **Step 3 (Red): Add three marker-file integration fixtures**

In `src-tauri/tests/harness_test.rs`, create isolated temporary repositories whose executable code writes marker files outside the repository only if spawned:

1. A `package.json` with `"test": "printf package-ran > ../package-ran"` and command `npm test`.
2. A Cargo project with `build.rs` writing `../build-hook-ran` and command `cargo check`.
3. A shell test script writing `../test-command-ran` and command `./test-command.sh`.

For every fixture:

- Invoke through `Harness::execute_tool_with_emitter`, not the classifier directly.
- Use each permission mode with a confirmation emitter that records the request and declines it.
- Assert exactly one explicit confirmation request contains the semantic reason.
- Assert the harness returns denial.
- Assert the outside marker does not exist.

Add separate hard-block fixtures for `printf x > ../outside.txt` and `rm -rf /`; assert no confirmation is emitted and no subprocess starts in all modes.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test harness_test project_defined -- --nocapture
```

Expected: tests fail under the old full-access/read-only policy.

- [ ] **Step 4 (Green): Make confirmation mandatory for semantic shell risk**

In the shell branch of `PermissionGate::check_with_evidence`:

```rust
match classify_shell_command(command, project_root) {
    ShellPolicyDecision::AllowInspection => automatic_allow(...),
    ShellPolicyDecision::RequireExplicitConfirmation { risk, reason } => {
        manual_confirmation_required(risk, reason)
    }
    ShellPolicyDecision::Block { reason } => hard_block(reason),
}
```

Do not branch on `PermissionMode` for `RequireExplicitConfirmation` or `Block`. Keep the existing mode behavior for non-shell tools.

In `Harness::execute_tool_with_emitter`, bind an accepted confirmation to the normalized command digest, pass the approval marker to execution, and discard it after that one attempt. A changed command must trigger a new decision.

- [ ] **Step 5 (Green): Register the exact command-safety acceptance label**

Add this gate under the existing `desktop-safety` domain:

```bash
add_gate 'desktop command execution safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test harness_test project_defined -- --nocapture && npm --prefix apps/desktop run check:backend'
```

Mirror it in `scripts/acceptance.test.mjs` in the same order.

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --only "desktop command execution safety baseline"
```

Expected: contract and selected gate pass.

- [ ] **Step 6 (Refactor): Prove the complete backend authority path remains green**

```bash
npm --prefix apps/desktop run check:backend
```

Expected: formatting, Clippy with warnings denied, and all Rust tests pass. No skipped malicious-fixture test is acceptable.

- [ ] **Step 7: Run change detection and commit**

Expected change scope: permission decision flow, focused integration tests, and acceptance metadata.

```bash
git add apps/desktop/src-tauri/src/harness/permissions_test.rs apps/desktop/src-tauri/tests/harness_test.rs apps/desktop/src-tauri/src/harness/mod.rs scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "test(desktop): prove shell safety across permission modes"
```

---

## Task Group 3: Credential And Persistent-Log Safety

### Task 5: Add An Injected Credential Store And macOS Keychain Backend

**Files:**

- Create: `apps/desktop/src-tauri/src/credential_store.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/Cargo.lock`

- [ ] **Step 1: Run impact analysis**

```text
impact({ repo: "forge", target: "AppState", direction: "upstream" })
impact({ repo: "forge", target: "AppState::new", direction: "upstream" })
impact({ repo: "forge", target: "detect_credentials", direction: "upstream" })
```

Expected: credential resolution reaches provider catalog/probe, session create/restore, and headless eval. Report HIGH/CRITICAL results before editing.

- [ ] **Step 2 (Red): Add store contract tests**

Create tests in `credential_store.rs` named:

- `memory_store_create_read_replace_delete`
- `credential_ref_uses_canonical_provider_account`
- `missing_credential_returns_none`
- `delete_missing_credential_is_idempotent`
- `unavailable_store_fails_create_read_and_delete`
- `credential_ref_debug_never_contains_secret`

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_store --lib
```

Expected: compile failure because the module and types do not exist.

- [ ] **Step 3 (Green): Implement the narrow interface**

Add `keyring = { version = "3", features = ["apple-native"] }` to macOS target dependencies and define:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialRef {
    pub service: String,
    pub account: String,
}

impl CredentialRef {
    pub const SERVICE: &'static str = "com.forge.desktop.provider";

    pub fn provider(provider: &str) -> Self {
        Self {
            service: Self::SERVICE.to_string(),
            account: format!("provider:{}", canonical_provider_id(provider)),
        }
    }

    pub fn profile(profile_id: &str, provider: &str) -> Self {
        Self {
            service: Self::SERVICE.to_string(),
            account: format!(
                "profile:{profile_id}:provider:{}",
                canonical_provider_id(provider)
            ),
        }
    }
}

pub trait CredentialStore: Send + Sync {
    fn put(&self, reference: &CredentialRef, secret: &str) -> Result<(), CredentialStoreError>;
    fn get(&self, reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError>;
    fn delete(&self, reference: &CredentialRef) -> Result<(), CredentialStoreError>;
}
```

Implement `KeychainCredentialStore` on macOS with `keyring::Entry::new`, `set_password`, `get_password`, and `delete_credential`; map `keyring::Error::NoEntry` to `Ok(None)`/idempotent delete. Map every other backend error to a bounded category without secret values. Implement `UnavailableCredentialStore` for unsupported construction and `MemoryCredentialStore` under `#[cfg(test)]`.

Add `pub(crate) credential_store: Arc<dyn CredentialStore>` to `AppState`, a production constructor using `system_credential_store()`, and `AppState::new_with_credential_store(...)` for tests. Wire `credential_migration::run_once(...)` from the existing Tauri setup in `apps/desktop/src-tauri/src/lib.rs` before session/provider restoration, and from headless startup in `state.rs`; do not expose the trait object through Tauri serialization.

- [ ] **Step 4 (Refactor): Run store tests and the AppState test set**

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_store --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml state --lib
```

Expected: all pass; compiler output contains no secret-bearing `Debug` implementation.

- [ ] **Step 5: Detect changes and commit**

```bash
git add apps/desktop/src-tauri/src/credential_store.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/state.rs apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "feat(desktop): add system credential store abstraction"
```

### Task 6: Centralize Redaction Before Any Persistent Write

**Files:**

- Create: `apps/desktop/src-tauri/src/redaction.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/logger.rs`
- Modify: `apps/desktop/src-tauri/src/log_store.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/openai_compatible.rs`
- Modify: `apps/desktop/src-tauri/src/provider_probe.rs`
- Modify: `apps/desktop/src-tauri/src/provider_model_catalog.rs`

- [ ] **Step 1: Run impact analysis**

```text
impact({ repo: "forge", target: "logger::log", direction: "upstream" })
impact({ repo: "forge", target: "LogStore::append", direction: "upstream" })
impact({ repo: "forge", target: "log_event", direction: "upstream" })
impact({ repo: "forge", target: "OpenAiCompatibleAdapter::stream_message_with_emitter", direction: "upstream" })
```

Expected: both `logger::log` and direct `log_event` callers persist through `LogStore`; the adapter currently logs a request-body prefix. Treat this as a high-sensitivity authority path.

- [ ] **Step 2 (Red): Add sentinel redaction tests**

Add tests proving all persisted forms remove `forge-secret-9d7f` from:

- `Authorization: Bearer forge-secret-9d7f`
- `x-api-key: forge-secret-9d7f`
- JSON keys `api_key`, `token`, `password`, `request_body`, `messages`, `system_prompt`, `hidden_context`, and `environment`
- URL query/fragment values
- a registered raw credential value embedded in free text
- plain `logger::log` output and direct `LogStore::append` output

Also add `structured_redaction_error_suppresses_persistence`: force the test redactor to return an error and assert neither `app.log` nor structured log storage gains an entry.

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml redaction --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml log_store --lib
```

Expected: compile failures before the redactor exists and leakage failures before integration.

- [ ] **Step 3 (Green): Implement one redactor and fail closed**

Define `PersistentLogRedactor` with:

```rust
pub fn register_secret(&self, value: &str);
pub fn redact_text(&self, input: &str) -> Result<String, RedactionError>;
pub fn redact_json(&self, input: &serde_json::Value) -> Result<serde_json::Value, RedactionError>;
```

Use case-insensitive key/header patterns, replace sensitive values with `[redacted]`, redact registered exact secret values, and drop URL query/fragment data. Never log a redaction error's input. `logger::log` must redact before writing `~/.forge/app.log`; `LogStore::append` must redact again as defense in depth. On error, persist nothing and emit only the constant stderr message `Forge log entry suppressed: redaction failed`.

Delete the OpenAI-compatible adapter's first-2,000-byte request-body log. Replace it with non-content metadata only:

```rust
logger::info(&format!(
    "provider request prepared provider={} message_count={} tool_count={} body_bytes={}",
    provider_id,
    messages.len(),
    tools.len(),
    body_len,
));
```

Make provider probe and model catalog call the central redactor instead of maintaining divergent local token/header sanitizers.

- [ ] **Step 4 (Refactor): Scan persistence call sites and run focused tests**

```bash
rg -n "write\(|write_all\(|append\(|fs::write" apps/desktop/src-tauri/src/logger.rs apps/desktop/src-tauri/src/log_store.rs apps/desktop/src-tauri/src/adapters apps/desktop/src-tauri/src/provider_probe.rs apps/desktop/src-tauri/src/provider_model_catalog.rs
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml redaction --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml log_store --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_probe --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_model_catalog --lib
```

Expected: every persistent log write is downstream of redaction; tests pass and temporary log files contain no sentinel.

- [ ] **Step 5: Detect changes and commit**

```bash
git add apps/desktop/src-tauri/src/redaction.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/logger.rs apps/desktop/src-tauri/src/log_store.rs apps/desktop/src-tauri/src/adapters/openai_compatible.rs apps/desktop/src-tauri/src/provider_probe.rs apps/desktop/src-tauri/src/provider_model_catalog.rs
git commit -m "fix(desktop): redact sensitive data before log persistence"
```

### Task 7: Migrate Settings And Profiles From Plaintext To Credential References

**Files:**

- Create: `apps/desktop/src-tauri/src/credential_migration.rs`
- Modify: `apps/desktop/src-tauri/src/settings.rs`
- Modify: `apps/desktop/src-tauri/src/profile/mod.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/settings_handlers.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/profile_handlers.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/handlers.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- Modify: `apps/desktop/src-tauri/src/provider_probe.rs`
- Modify: `apps/desktop/src-tauri/src/provider_model_catalog.rs`
- Modify: `apps/desktop/src-tauri/src/eval_headless/mod.rs`
- Modify: `apps/desktop/src-tauri/src/diagnostics/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src/lib/ipc/apiKeys.ts`
- Modify: `apps/desktop/src/lib/ipc/types.ts`
- Modify: `apps/desktop/src/components/settings/ProfileForm.tsx`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Run impact analysis for every credential entry point**

```text
impact({ repo: "forge", target: "detect_credentials", direction: "upstream" })
impact({ repo: "forge", target: "Settings::set_api_key", direction: "upstream" })
impact({ repo: "forge", target: "Settings::key_status", direction: "upstream" })
impact({ repo: "forge", target: "ProfileStore::upsert", direction: "upstream" })
impact({ repo: "forge", target: "ProfileStore::save", direction: "upstream" })
impact({ repo: "forge", target: "setup", file_path: "apps/desktop/src-tauri/src/lib.rs", direction: "upstream" })
impact({ repo: "forge", target: "AppState::new", file_path: "apps/desktop/src-tauri/src/state.rs", direction: "upstream" })
```

Expected direct consumers: provider probe/catalog, session create/restore, general handlers, diagnostics, and headless eval. Report high risk before edits.

- [ ] **Step 2 (Red): Add migration, serialization, and failure tests**

Add focused tests with temporary settings/profile files and `MemoryCredentialStore`:

- `legacy_settings_keys_migrate_to_verified_references_without_plaintext`
- `legacy_profile_overrides_migrate_to_verified_references_without_plaintext`
- `migration_is_idempotent_after_partial_file_completion`
- `store_write_failure_leaves_original_json_byte_for_byte`
- `store_readback_mismatch_leaves_original_json_byte_for_byte`
- `settings_save_never_serializes_api_keys`
- `profile_save_never_serializes_api_key_overrides`
- `setting_key_creates_reference_and_keychain_item`
- `deleting_key_removes_reference_and_keychain_item`
- `unavailable_store_prevents_provider_start_with_recovery_message`
- `key_status_reports_configured_without_returning_secret`

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_migration --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml profile --lib
```

Expected: failures because settings/profile models still serialize plaintext and resolution cannot report store failure.

- [ ] **Step 3 (Green): Change persisted schemas to references only**

Replace `Settings.api_keys` with:

```rust
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub credential_refs: HashMap<String, CredentialRef>,
```

Replace `ForgeProfile.api_key_overrides` with:

```rust
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub credential_overrides: HashMap<String, CredentialRef>,
```

Remove secret maps from `UpsertProfileInput` and all IPC response types. Legacy plaintext fields exist only in private migration-only serde structs; normal `Settings` and `ForgeProfile` cannot serialize them.

Introduce `CredentialResolver` backed by `Arc<dyn CredentialStore>`. Resolution returns `Result<Credentials, CredentialResolutionError>`, checks profile/provider references first, then existing Claude/env sources, and calls `redactor.register_secret` for every resolved secret before the provider can log. A credential-store error is not treated as “missing”; it returns a bounded recovery message and prevents provider start.

- [ ] **Step 4 (Green): Implement verified, idempotent migration**

For each legacy file independently:

1. Read bytes and deserialize a private legacy document.
2. Build deterministic references for all non-empty secrets.
3. Write every secret to the store.
4. Read every reference back and compare in memory.
5. Build the reference-only document.
6. Write a sibling temporary file, `sync_all`, preserve restrictive permissions, then rename atomically.
7. Re-read and assert no legacy secret field/value remains.

If steps 2–4 fail, leave the source bytes untouched. If settings succeeds and profiles fails, the next startup skips settings and retries profiles, making partial completion safe. Invoke migration during Tauri setup before provider/session restoration and during headless startup before adapter construction.

- [ ] **Step 5 (Green): Wire key create/status/delete without returning values**

Make settings IPC handlers accept secret input only for the write call, store it immediately, clear it from owned buffers where practical, and return `Result<(), String>`. `get_api_key_status` returns provider, configured/source/status, and bounded error only. Add a delete command or preserve the existing empty-string removal convention, but ensure it deletes both the Keychain item and reference.

Update the frontend profile/API-key types so no response property can contain a secret. Keep password inputs write-only and clear React state after success or failure.

- [ ] **Step 6 (Green): Add the exact combined credential/redaction gate**

```bash
add_gate 'desktop credential and redaction safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_store --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_migration --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml redaction --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml profile --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml log_store --lib && npm --prefix apps/desktop run check:backend'
```

Mirror the entry in `scripts/acceptance.test.mjs`, then run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --only "desktop credential and redaction safety baseline"
```

Expected: both pass and test temp files/logs contain no sentinel secret.

- [ ] **Step 7 (Refactor): Search the whole desktop tree for plaintext schema remnants**

```bash
rg -n "api_keys|api_key_overrides|request body preview|body_preview" apps/desktop/src-tauri/src apps/desktop/src apps/desktop/e2e
npm --prefix apps/desktop run build
npm --prefix apps/desktop run check:backend
```

Expected: matches are confined to private migration compatibility/tests or non-secret API-key status labels; frontend and Rust gates pass.

- [ ] **Step 8: Detect changes and commit**

```bash
git add apps/desktop/src-tauri/src/credential_migration.rs apps/desktop/src-tauri/src/settings.rs apps/desktop/src-tauri/src/profile/mod.rs apps/desktop/src-tauri/src/ipc/settings_handlers.rs apps/desktop/src-tauri/src/ipc/profile_handlers.rs apps/desktop/src-tauri/src/ipc/handlers.rs apps/desktop/src-tauri/src/ipc/session_lifecycle.rs apps/desktop/src-tauri/src/provider_probe.rs apps/desktop/src-tauri/src/provider_model_catalog.rs apps/desktop/src-tauri/src/eval_headless/mod.rs apps/desktop/src-tauri/src/diagnostics/mod.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/state.rs apps/desktop/src/lib/ipc/apiKeys.ts apps/desktop/src/lib/ipc/types.ts apps/desktop/src/components/settings/ProfileForm.tsx scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "fix(desktop): migrate provider secrets to system credentials"
```

---

## Task Group 4: Lossless Checkpoint Restore

### Task 8: Capture A Versioned Complete Git Snapshot

**Files:**

- Create: `apps/desktop/src-tauri/src/ipc/checkpoint_snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/mod.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/project_checkpoint.rs`

- [ ] **Step 1: Run impact analysis**

```text
impact({ repo: "forge", target: "create_project_checkpoint", direction: "upstream" })
impact({ repo: "forge", target: "checkpoint_is_restorable", direction: "upstream" })
impact({ repo: "forge", target: "snapshot_untracked_files", direction: "upstream" })
```

Expected: Project Archive/status and restore IPC depend on these symbols. Report high risk before editing.

- [ ] **Step 2 (Red): Add snapshot-capture tests**

Create disposable Git repositories and add tests for:

- staged-only text modification
- unstaged-only text modification
- staged plus unstaged changes to the same file
- staged rename and deletion
- tracked binary modification
- untracked binary file with arbitrary non-UTF-8 bytes
- unborn repository with no `HEAD`
- symlink and oversized untracked file recorded as unsupported

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::checkpoint_snapshot --lib
```

Expected: compile failure before the module exists.

- [ ] **Step 3 (Green): Define V2 snapshot state**

```rust
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeSnapshot {
    pub schema_version: u32,
    pub head_oid: Option<String>,
    pub status_porcelain_v2: String,
    pub staged_patch: String,
    pub unstaged_patch: String,
    pub untracked_files: Vec<SnapshotFile>,
    pub unsupported_paths: Vec<UnsupportedCheckpointPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub relative_path: String,
    pub bytes_base64: String,
    pub executable: bool,
}
```

Capture full `git rev-parse HEAD` when present, `git status --porcelain=v2 --untracked-files=all`, `git diff --cached --binary --full-index`, `git diff --binary --full-index`, and raw untracked bytes. Normalize and validate every relative path against the repository root. Preserve existing size limits, but put every skipped/symlink/special path into `unsupported_paths`; never call such a checkpoint restorable.

Keep a V1 deserializer only to return a clear non-destructive “legacy checkpoint must be recreated” error. Do not attempt destructive V1 restore.

- [ ] **Step 4 (Refactor): Verify complete capture and commit**

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::checkpoint_snapshot --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::project_checkpoint --lib
```

Expected: capture tests pass; existing project checkpoint tests are updated to schema V2.

```bash
git add apps/desktop/src-tauri/src/ipc/checkpoint_snapshot.rs apps/desktop/src-tauri/src/ipc/mod.rs apps/desktop/src-tauri/src/ipc/project_checkpoint.rs
git commit -m "feat(desktop): capture complete versioned git checkpoints"
```

### Task 9: Restore Atomically Or Refuse Before Mutation

**Files:**

- Modify: `apps/desktop/src-tauri/src/ipc/checkpoint_snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/project_checkpoint.rs`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Run impact analysis**

```text
impact({ repo: "forge", target: "restore_project_checkpoint", direction: "upstream" })
impact({ repo: "forge", target: "restore_checkpoint", direction: "upstream" })
impact({ repo: "forge", target: "apply_patch", direction: "upstream" })
impact({ repo: "forge", target: "cleanup_untracked_files", direction: "upstream" })
```

- [ ] **Step 2 (Red): Add round-trip, refusal, and rollback tests**

For every state from Task 8, capture checkpoint A, mutate to distinct state B, restore A, then assert exact `HEAD`, index bytes/modes, worktree bytes/modes, untracked paths/bytes, and porcelain-v2 status. Add tests that:

- refuse a checkpoint with unsupported paths before any mutation;
- refuse `HEAD` drift before mutation;
- inject failure while applying the unstaged patch and prove exact restoration of pre-call state B;
- inject failure while restoring an untracked file and prove exact restoration of state B;
- never use `git clean` against uncaptured paths.

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::project_checkpoint --lib
```

Expected: current implementation fails staged, combined, binary, rename/delete, and rollback cases.

- [ ] **Step 3 (Green): Implement preflight-first transactional restore**

Use this order exactly:

1. Load and validate schema, paths, base64, size limits, and `unsupported_paths`.
2. Require checkpoint `head_oid` to equal current full `HEAD` (including both `None` for unborn repositories).
3. Capture the complete current snapshot as rollback state; if it contains unsupported paths, refuse before mutation.
4. Reset tracked index/worktree to `HEAD` without deleting uncaptured files; for an unborn repository use `git read-tree --empty`, remove only paths represented in the captured rollback snapshot, and never invoke a `HEAD`-dependent reset.
5. Remove only untracked paths enumerated by the complete rollback snapshot.
6. Apply `staged_patch` using `git apply --binary --index --whitespace=nowarn -`.
7. Apply `unstaged_patch` using `git apply --binary --whitespace=nowarn -`.
8. Restore untracked bytes and executable modes using validated relative paths.
9. Capture again and compare semantic snapshot state to the checkpoint.
10. On any error after step 3, reset and materialize the rollback snapshot with the same primitive; return the original error plus bounded rollback status.

Never clean before successful preflight. Never shell-interpolate a path; pass it as an argument or use filesystem APIs after canonical boundary checks.

- [ ] **Step 4 (Green): Add the exact checkpoint gate**

```bash
add_gate 'desktop checkpoint restore safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::checkpoint_snapshot --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::project_checkpoint --lib && npm --prefix apps/desktop run check:backend'
```

Mirror it in `scripts/acceptance.test.mjs` and run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --only "desktop checkpoint restore safety baseline"
```

Expected: both pass.

- [ ] **Step 5 (Refactor): Run backend gate and commit**

```bash
npm --prefix apps/desktop run check:backend
git add apps/desktop/src-tauri/src/ipc/checkpoint_snapshot.rs apps/desktop/src-tauri/src/ipc/project_checkpoint.rs scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "fix(desktop): make checkpoint restore lossless and transactional"
```

---

## Task Group 5: CSP, Capabilities, Documentation, And R1 Closure

### Task 10: Harden Tauri CSP And Reduce Capabilities To Observed Use

**Files:**

- Create: `apps/desktop/scripts/desktop-security-config.test.mjs`
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/package-lock.json`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Run impact analysis and confirm observed frontend authority**

```text
impact({ repo: "forge", target: "useOutputStream", direction: "upstream" })
impact({ repo: "forge", target: "openProject", direction: "upstream" })
```

Then scan:

```bash
rg -n "@tauri-apps/(api|plugin)-(event|dialog|shell)|emit\(|listen\(|open\(" apps/desktop/src --glob '*.{ts,tsx}'
```

Expected observed browser needs: dialog `open`, event `listen`, and event `unlisten`. No frontend shell open or event emit use should remain.

- [ ] **Step 2 (Red): Add a static security configuration contract**

Create a Node test that parses both JSON files and asserts:

- production `app.security.csp` is a non-empty string with `default-src 'self'`, `script-src 'self'`, `object-src 'none'`, `base-uri 'none'`, and `frame-ancestors 'none'`;
- `style-src` allows only `'self'` and `'unsafe-inline'`;
- `img-src` allows only `'self'`, `data:`, and `blob:`;
- production `connect-src` contains only `'self'`, `ipc:`, and `http://ipc.localhost`;
- dev CSP adds only localhost/127.0.0.1 Vite HTTP/WebSocket origins;
- `freezePrototype` is true;
- capability permissions equal `dialog:allow-open`, `core:event:allow-listen`, and `core:event:allow-unlisten`;
- frontend source contains no `@tauri-apps/plugin-shell`, `emit(` from Tauri event APIs, remote script import, or `eval`/`new Function`.

Add:

```json
"check:security-config": "node --test scripts/desktop-security-config.test.mjs"
```

Run it. Expected: failure because CSP is null and permissions include `core:default`, `shell:allow-open`, and event emit grants.

- [ ] **Step 3 (Green): Set explicit production/dev CSP and least privilege**

Set production CSP to:

```text
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; frame-ancestors 'none'
```

Set dev CSP to the same policy plus `http://localhost:1420`, `http://127.0.0.1:1420`, `ws://localhost:1420`, and `ws://127.0.0.1:1420` in `connect-src`. Set `freezePrototype: true`.

Replace `capabilities/default.json` permissions with exactly:

```json
[
  "dialog:allow-open",
  "core:event:allow-listen",
  "core:event:allow-unlisten"
]
```

Remove the unused frontend `@tauri-apps/plugin-shell` dependency and update `package-lock.json` if `rg` confirms no import.

- [ ] **Step 4 (Green): Add the exact CSP/capability gate**

```bash
add_gate 'desktop CSP and capability safety baseline' 'npm --prefix apps/desktop run check:security-config && npm --prefix apps/desktop run build && npm --prefix apps/desktop run tauri -- build --no-bundle && npm --prefix apps/desktop run check:backend'
```

Mirror it in the ordered acceptance contract and restore the next gate's previous domain after this entry.

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --only "desktop CSP and capability safety baseline"
```

Expected: static security tests, frontend production build, and no-bundle packaged-app build pass.

- [ ] **Step 5 (Refactor): Smoke the two granted browser paths**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts --grep "open project|session output|dialog"
```

Expected: the injected IPC fixture proves the frontend still requests only dialog-open and event-listening paths; it is not packaged-app evidence. Packaged-app permission evidence remains a separate manual/distribution gate and must not be claimed by this R1 command.

- [ ] **Step 6: Detect changes and commit**

```bash
git add apps/desktop/scripts/desktop-security-config.test.mjs apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/capabilities/default.json scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "fix(desktop): enforce CSP and least-privilege capabilities"
```

### Task 11: Synchronize User Documentation And Prove The Complete R1 Gate Set

**Files:**

- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `apps/desktop/e2e/acceptance.spec.ts` only if final user-visible recovery wording needs smoke coverage

- [ ] **Step 1 (Red): Add documentation assertions to the relevant gate commands**

Extend, without renaming, the credential/redaction, checkpoint, and CSP acceptance commands with `rg -q` assertions for these exact phrases in all three docs:

- `macOS Keychain`
- `explicit confirmation for project-defined commands`
- `transactional checkpoint restore`
- `persistent log redaction`
- `least-privilege Tauri CSP`

Update `scripts/acceptance.test.mjs` to the same commands and run its contract test. Expected: selected gates fail until docs are updated.

- [ ] **Step 2 (Green): Document behavior and recovery, not implementation secrets**

Update root and desktop READMEs with:

- package/test/build commands prompt even in full-access mode because repository code may execute;
- credentials use macOS Keychain, legacy plaintext is migrated at startup, and Keychain denial blocks only the affected provider with a recovery instruction;
- persistent diagnostics omit request/prompt/hidden-context and credential values;
- checkpoint restore preserves staged/unstaged/untracked/binary state and refuses unsupported checkpoints before cleanup;
- Tauri browser authority is restricted to project dialog open and backend event listening.

Add one dated `CHANGELOG.md` entry covering the same user-visible changes and all five exact acceptance labels.

- [ ] **Step 3 (Green): Verify exact acceptance inventory**

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --list-json | node -e '
let input="";
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  const required = [
    "desktop deterministic signal cleanup",
    "desktop command execution safety baseline",
    "desktop credential and redaction safety baseline",
    "desktop checkpoint restore safety baseline",
    "desktop CSP and capability safety baseline",
  ];
  const labels = JSON.parse(input).map(entry => entry.label);
  for (const label of required) {
    if (labels.filter(candidate => candidate === label).length !== 1) process.exit(1);
  }
});'
```

Expected: contract passes and every required label appears exactly once.

- [ ] **Step 4 (Refactor): Run every safety gate, then the complete desktop/repository gates**

```bash
scripts/acceptance.sh --only "desktop deterministic signal cleanup"
scripts/acceptance.sh --only "desktop command execution safety baseline"
scripts/acceptance.sh --only "desktop credential and redaction safety baseline"
scripts/acceptance.sh --only "desktop checkpoint restore safety baseline"
scripts/acceptance.sh --only "desktop CSP and capability safety baseline"
npm run build:desktop
npm --prefix apps/desktop run check:frontend-architecture
npm --prefix apps/desktop run check:protocol
npm --prefix apps/desktop run check:backend
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
scripts/acceptance.sh --dry-run
```

Expected: every command exits zero; backend has no failed tests, Vite has no CSS warning, browser output has no React Query error, and dry-run advertises all five labels.

- [ ] **Step 5: Perform secret and workspace-boundary negative scans**

```bash
rg -n "Authorization:|Bearer |x-api-key|api_keys|api_key_overrides|request body" apps/desktop/src-tauri/src apps/desktop/src --glob '!**/*test*' --glob '!credential_migration.rs'
rg -n "AllowReadonly|npm test.*Allow|cargo test.*Allow" apps/desktop/src-tauri/src
rg -n '"csp"\s*:\s*null|core:default|shell:allow-open|core:event:allow-emit' apps/desktop/src-tauri
```

Expected: no plaintext persistence/logging, obsolete shell auto-allow, null CSP, or excessive capability matches. Review any legitimate header construction in memory; it must not be persisted.

- [ ] **Step 6: Run final GitNexus change detection and commit documentation**

Run `detect_changes({ repo: "forge", scope: "compare", base_ref: "main" })`. Review every affected execution flow against the five safety domains and record residual risk. No unexpected Eval Runner or website runtime symbol should be affected.

```bash
git add README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs apps/desktop/e2e/acceptance.spec.ts
git commit -m "docs(desktop): publish the R1 safety baseline"
```

Expected: only files actually changed are staged.

## Final Definition Of Done

- [ ] All five exact acceptance labels exist once and execute real condition checks rather than diagnostic no-ops.
- [ ] `npm --prefix apps/desktop run check:backend` passes the complete Rust suite.
- [ ] The production frontend build completes without unresolved Tailwind at-rule warnings.
- [ ] Mocked acceptance has no React Query undefined-data console error.
- [ ] Malicious package script, Rust build hook, and local test-command fixtures cannot create marker files without explicit approval in any mode.
- [ ] Catastrophic and external-workspace write fixtures are blocked without offering an approval bypass.
- [ ] Keychain unavailable/write/readback failures prevent the affected provider from starting and preserve legacy source bytes during migration.
- [ ] Normal settings/profile JSON and persistent logs contain no sentinel secret, request body, environment value, prompt, or hidden context.
- [ ] Checkpoint tests compare exact Git index/worktree/untracked bytes for text, rename/delete, and binary cases, including rollback after injected failure.
- [ ] Production/dev CSP and the three-item capability allowlist pass static and no-bundle packaged-build proof.
- [ ] README, desktop README, changelog, product acceptance smoke, and acceptance dry-run agree with runtime behavior.

## Plan Self-Review Commands

Run before handing this plan to an implementer:

```bash
rg -n '^### Task [0-9]+:' docs/superpowers/plans/2026-07-10-desktop-safety-baseline.md
rg -n '^\s*- \[ \]' docs/superpowers/plans/2026-07-10-desktop-safety-baseline.md
for label in \
  'desktop deterministic signal cleanup' \
  'desktop command execution safety baseline' \
  'desktop credential and redaction safety baseline' \
  'desktop checkpoint restore safety baseline' \
  'desktop CSP and capability safety baseline'; do
  rg -F "$label" docs/superpowers/plans/2026-07-10-desktop-safety-baseline.md >/dev/null || exit 1
done
rg -n 'TODO|TBD|fill this in|implement later|place''holder' docs/superpowers/plans/2026-07-10-desktop-safety-baseline.md
```

Expected: eleven numbered tasks, checkbox steps under every task, all five labels found, and the final unfinished-marker scan prints nothing.
