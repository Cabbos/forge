# Anthropic Small-Model Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the remote `qwen3.6-35b-a3b-nvfp4-fast` R1 lane reproducibly use Forge's Anthropic-compatible adapter, make failed tools count consistently, preserve canonical workspace identity, and prevent known dependency commands from touching user-protected lockfiles.

**Architecture:** Keep the existing provider adapters and shell-policy decision order. Add a named Anthropic R1 preset, one shared string-level tool outcome classifier, canonicalize candidate paths through their nearest existing ancestor, and feed high-confidence protected paths from the current user turn into a session-scoped permission preflight. Do not change vLLM, CC Switch state, eval forbidden metadata, or the general loop/convergence controller.

**Tech Stack:** Node.js test runner and Forge backtest script, Rust/Tokio, `regex`, Tauri event pipeline, Forge permission ledger, Cargo tests, GitNexus, remote vLLM Anthropic-compatible `/v1/messages`.

---

## Scope, risk, and file map

The approved design is `docs/superpowers/specs/2026-07-17-small-model-reliability-guardrails-design.md`. Preserve its non-goals: no CC Switch database writes, no vLLM restart, no OpenAI-compatible deletion, no eval-only forbidden metadata passed into the production Agent, no new action-novelty controller, and no concurrent remote cases.

GitNexus upstream impact measured before this plan:

| Symbol | Risk | Blast radius | Plan response |
|---|---|---:|---|
| `classify_shell_command_in_workspace` | HIGH | 53 symbols, 3 processes | Do not modify its public decision order or signature. |
| `PermissionGate::check_with_evidence` | MEDIUM | 32 symbols, 2 processes | Add one early, session-scoped protected-path branch and run the complete permission suite. |
| `completed_tool_trace` | MEDIUM | 5 direct callers | Preserve signature; delegate classification to one shared helper. |
| `path_is_within_workspace` | LOW | 7 symbols, 1 process | Make the canonical alias fix here only. |
| `record_tool_counts_emitter` | LOW | 1 direct caller, 1 process | Replace its duplicated classifier with the shared helper. |
| `buildBacktestPlan`, `parseArgs`, `runCli` | LOW | 2–3 symbols each | Add explicit non-secret runtime provider/model options. |

Before editing any existing symbol during execution, rerun its GitNexus upstream impact because the index and worktree may have changed. If any planned symbol is HIGH or CRITICAL at execution time, report that result before editing. The worktree is already dirty; inspect `git status --short` before every task, preserve unrelated changes, and commit only the paths named by that task.

Files and responsibilities:

- Modify `apps/desktop/scripts/run-forge-backtest.mjs`: accept and safely project the headless provider/model into a child process.
- Modify `apps/desktop/scripts/run-forge-backtest.test.mjs`: prove provider/model projection and secret-free dry-run output.
- Modify `apps/desktop/package.json`: add the named, serial remote Qwen R1 command.
- Create `apps/desktop/src-tauri/src/agent/protected_paths.rs`: parse only explicit current-turn protected path phrases.
- Modify `apps/desktop/src-tauri/src/agent/mod.rs`: register the protected-path module.
- Modify `apps/desktop/src-tauri/src/agent/session/loop.rs`: replace the protected set at the start of every turn.
- Modify `apps/desktop/src-tauri/src/agent/turn_state.rs`: own the shared tool-result classifier.
- Modify `apps/desktop/src-tauri/src/agent/turn_state_tests.rs`: cover non-zero shell results and unknown tools.
- Modify `apps/desktop/src-tauri/src/agent/session/tools.rs`: use the shared classifier for cumulative counts and loop progress.
- Modify `apps/desktop/src-tauri/src/agent/session/tools_test.rs`: cover batch failure counting.
- Modify `apps/desktop/src-tauri/src/harness/mod.rs`: give every permission-denied tool result a stable `Denied:` machine prefix.
- Modify `apps/desktop/src-tauri/src/harness/permissions.rs`: store per-session protected paths and enforce them before ordinary confirmation.
- Modify `apps/desktop/src-tauri/src/harness/permissions_test.rs`: cover protected direct writes, dependency commands, allowed checks, and session clearing.
- Modify `apps/desktop/src-tauri/src/harness/shell_policy.rs`: canonicalize aliases through the nearest existing ancestor and detect lockfile-mutating package-manager commands without changing the main classifier.
- Modify `apps/desktop/src-tauri/src/harness/permission_ledger.rs`: attach the protected file to permission evidence.
- Modify `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md`: document the runtime behavior and Anthropic R1 command.
- Modify `scripts/acceptance.sh` and `scripts/acceptance.test.mjs`: keep the permission acceptance gate aligned.
- Create `docs/superpowers/reports/2026-07-17-anthropic-small-model-reliability-report.md`: record local evidence and two serial remote R1 rounds.

### Task 1: Add a secret-safe Anthropic R1 preset

**Files:**
- Modify: `apps/desktop/scripts/run-forge-backtest.mjs`
- Modify: `apps/desktop/scripts/run-forge-backtest.test.mjs`
- Modify: `apps/desktop/package.json`

- [ ] **Step 1: Re-run impact analysis and inspect overlapping changes**

Run GitNexus upstream impact for `buildBacktestPlan`, `parseArgs`, and `runCli`, then run:

```bash
git status --short -- apps/desktop/scripts/run-forge-backtest.mjs apps/desktop/scripts/run-forge-backtest.test.mjs apps/desktop/package.json
git diff -- apps/desktop/scripts/run-forge-backtest.mjs apps/desktop/scripts/run-forge-backtest.test.mjs apps/desktop/package.json
```

Expected: LOW impact for the three script symbols. Preserve any unrelated edits shown by the diff.

- [ ] **Step 2: Write failing tests for runtime identity projection**

Add this test to `apps/desktop/scripts/run-forge-backtest.test.mjs`:

```js
test("plans a custom Anthropic headless runtime without exposing its secret", () => {
  const plan = buildBacktestPlan({
    repoRoot: "/repo/forge",
    runnerRoot: "/repo/forge/apps/eval-runner",
    suitePath: "/tmp/r1.json",
    outputPath: "/tmp/report.json",
    provider: "forge",
    model: "local-forge",
    headlessProvider: "custom_anthropic",
    headlessModel: "qwen3.6-35b-a3b-nvfp4-fast",
    env: { FORGE_CUSTOM_ANTHROPIC_API_KEY: "secret-token" },
  });

  assert.equal(plan.env.FORGE_HEADLESS_PROVIDER, "custom_anthropic");
  assert.equal(plan.env.FORGE_HEADLESS_MODEL, "qwen3.6-35b-a3b-nvfp4-fast");
  assert.equal(plan.env.FORGE_CUSTOM_ANTHROPIC_API_KEY, "secret-token");
  assert.doesNotMatch(JSON.stringify(plan.args), /secret-token/);
});
```

Add a CLI dry-run test that supplies a fake token and asserts stdout does not contain it:

```js
test("Anthropic R1 dry-run reports runtime identity but redacts credentials", () => {
  const scriptPath = resolve("scripts/run-forge-backtest.mjs");
  const result = spawnSync(
    process.execPath,
    [
      scriptPath,
      "--dry-run",
      "--case",
      "continuity-pipeline-due-date-labels",
      "--headless-provider",
      "custom_anthropic",
      "--headless-model",
      "qwen3.6-35b-a3b-nvfp4-fast",
    ],
    {
      cwd: resolve("."),
      encoding: "utf8",
      env: { ...process.env, FORGE_CUSTOM_ANTHROPIC_API_KEY: "secret-token" },
    },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /custom_anthropic/);
  assert.match(result.stdout, /qwen3\.6-35b-a3b-nvfp4-fast/);
  assert.doesNotMatch(result.stdout, /secret-token/);
});
```

- [ ] **Step 3: Run the tests and verify the new contract fails**

Run:

```bash
cd apps/desktop
node --test scripts/run-forge-backtest.test.mjs
```

Expected: FAIL because `buildBacktestPlan` does not accept `headlessProvider`/`headlessModel` and the CLI rejects `--headless-provider`.

- [ ] **Step 4: Implement explicit provider/model flags without logging secrets**

Extend `buildBacktestPlan`'s destructuring and environment projection in `apps/desktop/scripts/run-forge-backtest.mjs`:

```js
export function buildBacktestPlan({
  repoRoot,
  runnerRoot,
  suitePath,
  outputPath,
  provider,
  model,
  headlessProvider,
  headlessModel,
  env = process.env,
}) {
  const planEnv = { ...env };
  if (headlessProvider) planEnv.FORGE_HEADLESS_PROVIDER = headlessProvider;
  if (headlessModel) planEnv.FORGE_HEADLESS_MODEL = headlessModel;
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
```

Add `headlessProvider` and `headlessModel` to the options object, parse `--headless-provider` and `--headless-model`, and pass them to `buildBacktestPlan`. In dry-run JSON, expose only:

```js
runtime: {
  provider: plan.env.FORGE_HEADLESS_PROVIDER ?? null,
  model: plan.env.FORGE_HEADLESS_MODEL ?? null,
  customAnthropicBaseConfigured: Boolean(plan.env.FORGE_CUSTOM_ANTHROPIC_BASE_URL),
  customAnthropicCredentialConfigured: Boolean(plan.env.FORGE_CUSTOM_ANTHROPIC_API_KEY),
},
```

Never print the base URL query string, authorization token, or the complete child environment.

- [ ] **Step 5: Add the named serial R1 command**

Add this script to `apps/desktop/package.json`:

```json
"eval:qwen:r1:anthropic": "node scripts/run-forge-backtest.mjs --case continuity-pipeline-storage-validation,continuity-pipeline-priority-labels,continuity-pipeline-due-date-labels --headless-provider custom_anthropic --headless-model qwen3.6-35b-a3b-nvfp4-fast --require-key --timeout 900 --max-model-rounds 20"
```

This command forms one suite and the eval runner executes its tasks serially. Do not add worker parallelism or a fallback provider.

- [ ] **Step 6: Run the focused tests and dry-run**

Run:

```bash
cd apps/desktop
node --test scripts/run-forge-backtest.test.mjs
npm run eval:qwen:r1:anthropic -- --dry-run
cargo test --manifest-path src-tauri/Cargo.toml adapters::provider_conformance --lib
cargo test --manifest-path src-tauri/Cargo.toml adapters::tests::build_adapter_routes_registry_providers_by_capability --lib
```

Expected: PASS; dry-run lists the three case IDs, `custom_anthropic`, the Qwen model, and only booleans for endpoint/credential presence. Existing conformance coverage continues to prove that `custom_anthropic` selects the Anthropic-compatible adapter family and retains its configured base URL.

- [ ] **Step 7: Commit the preset**

```bash
git add apps/desktop/scripts/run-forge-backtest.mjs apps/desktop/scripts/run-forge-backtest.test.mjs apps/desktop/package.json
git commit -m "feat(eval): add serial anthropic qwen r1 preset"
```

### Task 2: Unify tool failure classification and loop progress

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/turn_state.rs`
- Modify: `apps/desktop/src-tauri/src/agent/turn_state_tests.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools_test.rs`

- [ ] **Step 1: Re-run impact analysis**

Run upstream impact for `is_errorish_tool_result`, `completed_tool_trace`, and `record_tool_counts_emitter`. Expected: LOW, MEDIUM, and LOW respectively. Stop and report if the refreshed result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing classifier tests**

Add to `apps/desktop/src-tauri/src/agent/turn_state_tests.rs`:

```rust
#[test]
fn shared_tool_result_classifier_handles_shell_exit_and_unknown_tool() {
    assert!(crate::agent::turn_state::tool_result_is_error(
        "run_shell",
        "Exit code: 1\nStdout:\n\nStderr:\nfailed"
    ));
    assert!(!crate::agent::turn_state::tool_result_is_error(
        "run_shell",
        "Exit code: 0\nStdout:\nok\nStderr:\n"
    ));
    assert!(crate::agent::turn_state::tool_result_is_error(
        "write_to_",
        "Unknown tool: write_to_"
    ));
    assert!(crate::agent::turn_state::tool_result_is_error(
        "mcp__missing__tool",
        "Unknown MCP tool: mcp__missing__tool"
    ));
}
```

Add to `apps/desktop/src-tauri/src/agent/session/tools_test.rs` and import `count_failed_tool_results`:

```rust
#[test]
fn failed_batch_count_uses_tool_name_and_shell_exit_code() {
    let calls = vec![
        tc("run_shell", serde_json::json!({"command": "npm test"})),
        tc("write_to_", serde_json::json!({})),
        tc("read_file", serde_json::json!({"path": "package.json"})),
    ];
    let results = std::collections::HashMap::from([
        (calls[0].id.clone(), "Exit code: 1\nStderr:\nfailed".to_string()),
        (calls[1].id.clone(), "Unknown tool: write_to_".to_string()),
        (calls[2].id.clone(), "{}".to_string()),
    ]);

    assert_eq!(count_failed_tool_results(&calls, &results), 2);
}
```

- [ ] **Step 3: Run focused tests and confirm red state**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shared_tool_result_classifier_handles_shell_exit_and_unknown_tool --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml failed_batch_count_uses_tool_name_and_shell_exit_code --lib
```

Expected: FAIL because both new functions are absent.

- [ ] **Step 4: Implement the shared classifier**

Replace the split classification in `apps/desktop/src-tauri/src/agent/turn_state.rs` with:

```rust
pub(crate) fn tool_result_is_error(tool_name: &str, result: &str) -> bool {
    if classify_tool_category(tool_name) == AgentToolCategory::Shell {
        if let Some(exit_code) = shell_exit_code(result) {
            return exit_code != 0;
        }
    }
    is_errorish_tool_result(result)
}

pub(crate) fn is_errorish_tool_result(result: &str) -> bool {
    result.starts_with("Error:")
        || result.starts_with("Denied:")
        || result.starts_with("Search blocked:")
        || result.starts_with("Search failed:")
        || result.starts_with("Search timed out")
        || result.starts_with("Permission denied")
        || result.starts_with("Tool disabled")
        || result.starts_with("Tool execution blocked")
        || result.starts_with("Tool result missing:")
        || result.starts_with("Unknown tool:")
        || result.starts_with("Unknown MCP tool:")
        || result.to_ascii_lowercase().contains("file not found")
}
```

In `completed_tool_trace`, calculate:

```rust
let category = classify_tool_category(&name);
let is_error = tool_result_is_error(&name, result);
```

Keep the existing public function signature and trace status projection unchanged.

- [ ] **Step 5: Share the classifier with cumulative counts**

In `apps/desktop/src-tauri/src/agent/session/tools.rs`, import `tool_result_is_error` and add:

```rust
pub(crate) fn count_failed_tool_results(
    tool_calls: &[crate::adapters::base::ToolCall],
    result_map: &std::collections::HashMap<String, String>,
) -> usize {
    tool_calls
        .iter()
        .filter(|tool_call| {
            result_map
                .get(&tool_call.id)
                .map(|result| tool_result_is_error(&tool_call.name, result))
                .unwrap_or(true)
        })
        .count()
}
```

Then replace the duplicated filter in `record_tool_counts_emitter` with:

```rust
let batch_total = tool_calls.len();
let batch_failed = count_failed_tool_results(tool_calls, result_map);
```

Retain `made_progress = batch_failed < batch_total`; after this change, a batch containing only failed tools is deterministically no-progress.

- [ ] **Step 6: Run focused and module tests**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_state_tests --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::tools_test --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session --lib
```

Expected: PASS, including existing shell exit 0/1 trace tests.

- [ ] **Step 7: Commit the classifier**

```bash
git add apps/desktop/src-tauri/src/agent/turn_state.rs apps/desktop/src-tauri/src/agent/turn_state_tests.rs apps/desktop/src-tauri/src/agent/session/tools.rs apps/desktop/src-tauri/src/agent/session/tools_test.rs
git commit -m "fix(agent): count failed tool outcomes consistently"
```

### Task 3: Give permission denials a stable machine prefix

**Files:**
- Modify: `apps/desktop/src-tauri/src/harness/mod.rs`
- Modify: `apps/desktop/src-tauri/src/harness/permissions_test.rs`

- [ ] **Step 1: Run impact analysis for the harness denial branch**

Run context and upstream impact for `Harness::execute_tool_with_emitter`. Review all direct callers; do not change its return type.

- [ ] **Step 2: Write the failing stable-prefix test**

Add `use super::super::denied_tool_result;` to the test module imports in `apps/desktop/src-tauri/src/harness/permissions_test.rs`, then add:

```rust
#[test]
fn permission_denial_tool_result_has_stable_machine_prefix() {
    assert_eq!(
        denied_tool_result("已阻止：这条命令可能写入工作区之外。"),
        "Denied: 已阻止：这条命令可能写入工作区之外。"
    );
}
```

- [ ] **Step 3: Verify the test fails**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_denial_tool_result_has_stable_machine_prefix --lib
```

Expected: FAIL because `denied_tool_result` is absent.

- [ ] **Step 4: Implement the formatter and use it only for tool results**

Add near the harness execution pipeline:

```rust
pub(crate) fn denied_tool_result(reason: &str) -> String {
    if reason.starts_with("Denied:") {
        reason.to_string()
    } else {
        format!("Denied: {reason}")
    }
}
```

In the `PermissionDecision::Deny { reason }` branch, keep permission evidence's localized `reason` unchanged, but use a separate result:

```rust
let result = denied_tool_result(&reason);
emit_blocked_tool_result_with_emitter(session_id, tool_block_id, &result, &*emitter);
self.dispatch_post_tool_event(session_id, tool_name, result.clone())
    .await;
return result;
```

This avoids classifying translated display text while preserving the existing user-facing permission evidence.

- [ ] **Step 5: Run harness and classifier tests**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_state_tests --lib
```

Expected: PASS; a Chinese shell-policy denial returned to the model now starts with `Denied:` and is counted as failed.

- [ ] **Step 6: Commit the stable denial contract**

```bash
git add apps/desktop/src-tauri/src/harness/mod.rs apps/desktop/src-tauri/src/harness/permissions_test.rs
git commit -m "fix(harness): mark permission denials as failed tool results"
```

### Task 4: Reproduce and fix canonical workspace aliases

**Files:**
- Modify: `apps/desktop/src-tauri/src/harness/shell_policy.rs`

- [ ] **Step 1: Re-run path impact and avoid the HIGH-risk classifier symbol**

Run upstream impact for `path_is_within_workspace`. Expected: LOW. Do not edit `classify_shell_command_in_workspace` in this task.

- [ ] **Step 2: Add a red alias test while retaining the escape test**

Inside the existing Unix-only shell-policy tests, add:

```rust
#[cfg(unix)]
#[test]
fn equivalent_workspace_alias_is_not_treated_as_external() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("root");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    fs::write(workspace.join("README.md"), "forge").expect("fixture");
    let alias = root.path().join("workspace-alias");
    symlink(&workspace, &alias).expect("alias");
    let command = format!("cp README.md {}/COPY.md", alias.display());

    assert_eq!(
        classify(&command, &workspace),
        ShellPolicyDecision::RequireExplicitConfirmation {
            risk: ShellRisk::Normal,
            reason: ShellPolicyReason::DangerousMutation,
        }
    );
}
```

The important assertion is that the alias is an in-workspace mutation requiring confirmation, not an `ExternalWrite` hard block. Keep `symlink_paths_cannot_escape_the_workspace` unchanged to prove an in-workspace symlink pointing outward is still rejected.

- [ ] **Step 3: Run the alias and escape tests**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml equivalent_workspace_alias_is_not_treated_as_external --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml symlink_paths_cannot_escape_the_workspace --lib
```

Expected before the fix: alias test FAIL with `Block { reason: ExternalWrite }`; escape test PASS.

- [ ] **Step 4: Canonicalize the nearest existing ancestor before containment comparison**

Add this private helper in `apps/desktop/src-tauri/src/harness/shell_policy.rs`:

```rust
pub(crate) fn canonicalize_workspace_path(path: &Path) -> Option<PathBuf> {
    let mut existing = normalize_lexically(path);
    let mut suffix = Vec::new();
    while !existing.exists() {
        suffix.push(existing.file_name()?.to_os_string());
        if !existing.pop() {
            return None;
        }
    }

    let mut canonical = existing.canonicalize().ok()?;
    for part in suffix.iter().rev() {
        canonical.push(part);
    }
    Some(normalize_lexically(&canonical))
}
```

Replace `path_is_within_workspace` with:

```rust
fn path_is_within_workspace(candidate: &Path, workspace_root: &Path) -> bool {
    let Some(workspace_root) = canonicalize_workspace_path(workspace_root) else {
        return false;
    };
    let Some(candidate) = canonicalize_workspace_path(candidate) else {
        return false;
    };
    candidate.starts_with(workspace_root)
}
```

This handles `/var` versus `/private/var`, aliases to an existing workspace, and not-yet-created files. It remains fail-closed when the nearest ancestor cannot be resolved, and it resolves outward symlinks before comparing.

- [ ] **Step 5: Run the full shell policy suite**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
```

Expected: PASS for the alias, `..` external path, symlink escape, catastrophic command, and ordinary project execution cases.

- [ ] **Step 6: Commit the identity fix**

```bash
git add apps/desktop/src-tauri/src/harness/shell_policy.rs
git commit -m "fix(harness): canonicalize workspace path aliases"
```

### Task 5: Parse high-confidence protected paths from each user turn

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/protected_paths.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop_test.rs`

- [ ] **Step 1: Re-run impact for `AgentSession::setup_turn`**

Expected: LOW. Confirm the method still receives the unmodified current user text.

- [ ] **Step 2: Add parser tests before registering the module**

Create `apps/desktop/src-tauri/src/agent/protected_paths.rs` with tests first:

```rust
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(test)]
mod tests {
    use super::explicit_protected_paths;
    use std::path::Path;

    #[test]
    fn extracts_only_explicit_chinese_and_english_paths() {
        let paths = explicit_protected_paths(
            "更新 package.json。不要修改 .env、.forge、package-lock.json。do not modify src/styles.css.",
            Path::new("/workspace"),
        );
        assert_eq!(
            paths,
            vec![
                "/workspace/.env".into(),
                "/workspace/.forge".into(),
                "/workspace/package-lock.json".into(),
                "/workspace/src/styles.css".into(),
            ]
        );
    }

    #[test]
    fn ignores_wildcards_urls_and_non_path_prose() {
        let paths = explicit_protected_paths(
            "不要修改任何配置，也不要修改 src/*.ts；do not modify https://example.com/a.",
            Path::new("/workspace"),
        );
        assert!(paths.is_empty());
    }

    #[test]
    fn supports_the_explicit_bude_gaidong_phrase() {
        let paths = explicit_protected_paths(
            "不得改动 pnpm-lock.yaml。",
            Path::new("/workspace"),
        );
        assert_eq!(paths, vec!["/workspace/pnpm-lock.yaml".into()]);
    }
}
```

- [ ] **Step 3: Register the empty implementation and confirm the tests fail**

Add `pub(crate) mod protected_paths;` to `apps/desktop/src-tauri/src/agent/mod.rs`. Add this temporary implementation above the tests:

```rust
pub(crate) fn explicit_protected_paths(_text: &str, _workspace: &Path) -> Vec<PathBuf> {
    Vec::new()
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::protected_paths --lib
```

Expected: two extraction tests FAIL and the ambiguity test PASS.

- [ ] **Step 4: Implement the high-confidence parser**

Replace the empty implementation with:

```rust
fn protected_clause_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)(?:不要修改|不得改动|do not modify)\s+([^。；;!！?？\n]+)",
        )
        .expect("protected path regex")
    })
}

fn explicit_path_token(token: &str) -> Option<&str> {
    let token = token.trim().trim_matches(|ch| matches!(ch, '`' | '\'' | '"'));
    let token = token.strip_suffix('.').unwrap_or(token);
    if token.is_empty()
        || token.contains('*')
        || token.contains("..")
        || token.contains("://")
        || token.chars().any(char::is_whitespace)
        || !(token.contains('.') || token.contains('/'))
    {
        return None;
    }
    Some(token)
}

pub(crate) fn explicit_protected_paths(text: &str, workspace: &Path) -> Vec<PathBuf> {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut paths = protected_clause_regex()
        .captures_iter(text)
        .filter_map(|capture| capture.get(1))
        .flat_map(|capture| capture.as_str().split(['、', ',']))
        .filter_map(explicit_path_token)
        .map(|token| {
            let path = PathBuf::from(token);
            if path.is_absolute() { path } else { workspace.join(path) }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}
```

- [ ] **Step 5: Store the current-turn set during setup**

Add this separate failing test to `apps/desktop/src-tauri/src/agent/session/loop_test.rs`:

```rust
#[tokio::test]
async fn setup_turn_replaces_current_protected_paths() {
    let workspace = temp_workspace("protected-paths");
    let adapter = Arc::new(QueuedAdapter::new(vec![]));
    let session = make_session(&workspace, adapter);
    let emitter = Arc::new(CollectingEventEmitter::new());

    session
        .setup_turn(
            "实现功能，不要修改 package-lock.json。",
            vec![],
            None,
            None,
            &*emitter,
        )
        .await;
    let first_turn = session
        .harness
        .permission_gate
        .protected_paths_for_session(&session.id)
        .await;
    assert_eq!(first_turn, vec![workspace.join("package-lock.json")]);

    session
        .setup_turn("继续检查", vec![], None, None, &*emitter)
        .await;
    let second_turn = session
    .harness
    .permission_gate
    .protected_paths_for_session(&session.id)
    .await;
    assert!(second_turn.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}
```

Run the test and expect failure because the permission API does not exist yet; Task 6 adds it. Do not parse eval `forbidden_files_changed` metadata or place protected paths in model-visible messages.

- [ ] **Step 6: Commit the parser only after Task 6 completes the storage API**

Do not commit a test that cannot compile. Continue directly into Task 6, then commit the parser, setup wiring, permission storage, and their tests together.

### Task 6: Enforce protected paths before shell confirmation

**Files:**
- Modify: `apps/desktop/src-tauri/src/harness/permissions.rs`
- Modify: `apps/desktop/src-tauri/src/harness/permissions_test.rs`
- Modify: `apps/desktop/src-tauri/src/harness/shell_policy.rs`
- Modify: `apps/desktop/src-tauri/src/harness/permission_ledger.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop_test.rs`
- Create: `apps/desktop/src-tauri/src/agent/protected_paths.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`

- [ ] **Step 1: Re-run impact for the permission symbols**

Run upstream impact for `PermissionGate::check_with_evidence`, `PermissionGate::clear_session`, and `PermissionLedgerEvent::decision`. Expected maximum risk: MEDIUM. Do not modify HIGH-risk `classify_shell_command_in_workspace`.

- [ ] **Step 2: Add failing permission preflight tests**

In `apps/desktop/src-tauri/src/harness/permissions_test.rs`, add async tests with a temporary workspace and `PermissionGate`:

```rust
#[tokio::test]
async fn protected_lockfile_blocks_dependency_mutation_before_confirmation() {
    let (db, workspace) = temp_db();
    let gate = PermissionGate::new(db);
    gate.set_turn_protected_paths(
        "session-1",
        vec![workspace.join("package-lock.json")],
    )
    .await;

    for command in ["npm install", "npm add left-pad", "npm remove left-pad", "npm update"] {
        let check = gate
            .check_with_evidence(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": command}),
                &workspace,
            )
            .await;
        assert!(matches!(check.decision, PermissionDecision::Deny { .. }), "{command}");
        assert_eq!(check.evidence.reason, "explicit_protected_path");
        assert_eq!(check.evidence.affected_files, vec!["package-lock.json"]);
    }
}

#[tokio::test]
async fn protected_lockfile_does_not_block_checks_or_unprotected_installs() {
    let (db, workspace) = temp_db();
    let gate = PermissionGate::new(db);
    gate.set_turn_protected_paths(
        "session-1",
        vec![workspace.join("package-lock.json")],
    )
    .await;

    for command in ["npm test", "npx tsc --noEmit"] {
        let decision = gate
            .check("session-1", "run_shell", &serde_json::json!({"command": command}), &workspace)
            .await;
        assert!(matches!(decision, PermissionDecision::Ask { .. }), "{command}");
    }

    let decision = gate
        .check("session-2", "run_shell", &serde_json::json!({"command": "npm install"}), &workspace)
        .await;
    assert!(matches!(decision, PermissionDecision::Ask { .. }));
}

#[tokio::test]
async fn direct_write_to_protected_path_is_denied_and_session_clear_removes_constraint() {
    let (db, workspace) = temp_db();
    let gate = PermissionGate::new(db);
    gate.set_turn_protected_paths(
        "session-1",
        vec![workspace.join("package-lock.json")],
    )
    .await;

    let denied = gate
        .check("session-1", "write_to_file", &serde_json::json!({"path": "package-lock.json"}), &workspace)
        .await;
    assert!(matches!(denied, PermissionDecision::Deny { .. }));

    gate.clear_session("session-1").await;
    assert!(gate.protected_paths_for_session("session-1").await.is_empty());
}
```

- [ ] **Step 3: Add failing shell semantic tests**

Expose a crate-private helper from `apps/desktop/src-tauri/src/harness/shell_policy.rs` and test:

```rust
#[test]
fn dependency_commands_report_the_lockfiles_they_may_modify() {
    assert_eq!(
        dependency_lockfiles_may_change("npm install"),
        vec!["package-lock.json"]
    );
    assert_eq!(
        dependency_lockfiles_may_change("pnpm add zod"),
        vec!["pnpm-lock.yaml"]
    );
    assert_eq!(
        dependency_lockfiles_may_change("yarn remove zod"),
        vec!["yarn.lock"]
    );
    assert_eq!(
        dependency_lockfiles_may_change("bun update"),
        vec!["bun.lock", "bun.lockb"]
    );
    assert!(dependency_lockfiles_may_change("npm test").is_empty());
    assert!(dependency_lockfiles_may_change("npx tsc --noEmit").is_empty());
}
```

- [ ] **Step 4: Verify the protected-path tests fail**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml protected_lockfile --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml direct_write_to_protected_path --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dependency_commands_report_the_lockfiles --lib
```

Expected: FAIL because storage and package-manager classification are absent.

- [ ] **Step 5: Implement narrow dependency-command detection**

In `apps/desktop/src-tauri/src/harness/shell_policy.rs`, add:

```rust
pub(crate) fn dependency_lockfiles_may_change(command: &str) -> Vec<&'static str> {
    let tokens = shell_tokens(command);
    let Some((program, args)) = program_and_args(&tokens) else {
        return Vec::new();
    };
    let Some(subcommand) = args.first().map(String::as_str) else {
        return Vec::new();
    };
    let mutating = matches!(
        subcommand,
        "install" | "i" | "add" | "remove" | "rm" | "uninstall" | "update" | "upgrade"
    );
    if !mutating {
        return Vec::new();
    }
    match program.as_str() {
        "npm" => vec!["package-lock.json"],
        "pnpm" => vec!["pnpm-lock.yaml"],
        "yarn" => vec!["yarn.lock"],
        "bun" => vec!["bun.lock", "bun.lockb"],
        _ => Vec::new(),
    }
}
```

Do not include `npm test`, `npm run`, `npx`, package-manager read commands, or arbitrary shell commands. `npm ci` is intentionally excluded because its contract is lockfile-preserving; if a real trace proves otherwise, add a separate red test before changing this list.

- [ ] **Step 6: Store normalized per-session protected paths**

Add to `PermissionGate`:

```rust
turn_protected_paths: RwLock<HashMap<String, HashSet<PathBuf>>>,
```

Initialize it in `PermissionGate::new`, then add:

```rust
pub async fn set_turn_protected_paths(&self, session_id: &str, paths: Vec<PathBuf>) {
    let normalized = paths
        .into_iter()
        .map(|path| crate::harness::shell_policy::canonicalize_workspace_path(&path).unwrap_or(path))
        .collect::<HashSet<_>>();
    self.turn_protected_paths
        .write()
        .await
        .insert(session_id.to_string(), normalized);
}

pub async fn protected_paths_for_session(&self, session_id: &str) -> Vec<PathBuf> {
    let mut paths = self
        .turn_protected_paths
        .read()
        .await
        .get(session_id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
```

Reuse the `pub(crate) canonicalize_workspace_path` helper created in Task 4; do not create a second canonicalization algorithm. Extend `clear_session` with:

```rust
self.turn_protected_paths.write().await.remove(session_id);
```

- [ ] **Step 7: Enforce direct writes and package-manager mutations**

Add private helpers in `permissions.rs` that compare canonical paths and return the matched protected path. At the start of the `write_to_file | edit_file` branch, after confirming the target is inside the workspace but before trust/full-access shortcuts, deny an exact protected target.

At the start of the `run_shell` branch, before `classify_shell_command_in_workspace`, intersect `dependency_lockfiles_may_change(command)` with the session's protected basenames. On a match, return:

```rust
let relative = matched
    .strip_prefix(working_dir)
    .unwrap_or(&matched)
    .to_string_lossy()
    .into_owned();
let reason = format!(
    "Protected path `{relative}` cannot be modified by this dependency command. Use existing dependencies or a no-install verification command."
);
let evidence = PermissionLedgerEvent::decision(
    PermissionLedgerEventKind::BlockedPolicy,
    session_id,
    canonical,
    input,
    working_dir,
    permission_mode,
    "explicit_protected_path",
)
.with_affected_files(vec![relative]);
return self
    .record_check(PermissionDecision::Deny { reason }, evidence)
    .await;
```

Add this method to `PermissionLedgerEvent`:

```rust
pub fn with_affected_files(mut self, affected_files: Vec<String>) -> Self {
    self.affected_files = affected_files;
    self
}
```

Do not auto-rewrite the command, auto-install dependencies, or bypass the existing confirmation path when no protected lockfile matches.

- [ ] **Step 8: Wire the current user turn into the gate**

In `AgentSession::setup_turn`, before building the model prompt, add:

```rust
let protected_paths = crate::agent::protected_paths::explicit_protected_paths(
    text,
    &self.harness.working_dir,
);
self.harness
    .permission_gate
    .set_turn_protected_paths(&self.id, protected_paths)
    .await;
```

This replaces the prior turn's set even when the new set is empty. Repair prompts already include the original task, so constraints are re-established without reading eval-only fields.

- [ ] **Step 9: Run parser, permission, shell, and session tests**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::protected_paths --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::loop_test --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
```

Expected: PASS. `npm install` is denied only when `package-lock.json` is in that session's current protected set; `npm test`, `npx tsc --noEmit`, another session, and a later unprotected turn retain existing behavior.

- [ ] **Step 10: Commit the protected-path preflight**

```bash
git add apps/desktop/src-tauri/src/agent/protected_paths.rs apps/desktop/src-tauri/src/agent/mod.rs apps/desktop/src-tauri/src/agent/session/loop.rs apps/desktop/src-tauri/src/agent/session/loop_test.rs apps/desktop/src-tauri/src/harness/permissions.rs apps/desktop/src-tauri/src/harness/permissions_test.rs apps/desktop/src-tauri/src/harness/shell_policy.rs apps/desktop/src-tauri/src/harness/permission_ledger.rs
git commit -m "feat(agent): enforce explicit protected lockfiles"
```

### Task 7: Align documentation and acceptance coverage

**Files:**
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Inspect dirty documentation before editing**

```bash
git status --short -- README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs
git diff -- README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs
```

Expected: possibly unrelated edits from other work. Integrate without replacing them.

- [ ] **Step 2: Extend the permission acceptance command first**

In `scripts/acceptance.sh`, extend `desktop command execution safety baseline` so it runs the new outcome/parser tests in addition to existing shell and permission tests:

```bash
add_gate 'desktop command execution safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::protected_paths --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_state_tests --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test harness_test project_defined -- --nocapture && npm --prefix apps/desktop run check:backend'
```

Update the exact matching expected command in `scripts/acceptance.test.mjs`.

- [ ] **Step 3: Run the acceptance contract and observe any mismatch**

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run --only 'desktop command execution safety baseline'
```

Expected: PASS after both command copies match exactly; the dry-run must advertise the new parser and outcome suites.

- [ ] **Step 4: Document the behavior and operator command**

Add concise, consistent text to all three docs:

```markdown
- Remote Qwen R1 evaluation uses the named `eval:qwen:r1:anthropic` preset. The operator supplies `FORGE_CUSTOM_ANTHROPIC_BASE_URL` and `FORGE_CUSTOM_ANTHROPIC_API_KEY` only through the child environment; the preset is serial and does not fall back to another protocol.
- Tool failures use shell exit codes plus stable executor prefixes, so turn metrics, loop progress, continuity evidence, and eval traces agree.
- Explicit current-turn phrases such as `不要修改 package-lock.json` and `do not modify package-lock.json` create a session-scoped protected file. Known install/add/remove/update commands are denied before confirmation when they may modify that lockfile.
```

In `CHANGELOG.md`, place the entries under the current unreleased section. In `README.md` and `apps/desktop/README.md`, place the operator command next to the existing permission/eval instructions. Do not include the real endpoint, CC Switch database path, token, or machine-specific provider UUID.

- [ ] **Step 5: Run documentation and dry-run checks**

```bash
rg -n "eval:qwen:r1:anthropic|protected file|保护文件|tool failures|工具失败" README.md apps/desktop/README.md CHANGELOG.md
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Expected: each behavior appears in the required docs and the acceptance matrix remains valid.

- [ ] **Step 6: Commit docs and acceptance**

```bash
git add README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "docs: document anthropic small-model reliability"
```

### Task 8: Run local gates, two serial remote R1 rounds, and write the report

**Files:**
- Create: `docs/superpowers/reports/2026-07-17-anthropic-small-model-reliability-report.md`
- Create: `apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json`
- Create: `apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-2.json`

- [ ] **Step 1: Run local focused and full backend gates**

```bash
node --test apps/desktop/scripts/run-forge-backtest.test.mjs scripts/acceptance.test.mjs
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_state_tests --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
scripts/acceptance.sh --only 'desktop command execution safety baseline'
scripts/acceptance.sh --dry-run
```

Expected: all commands PASS. If a failure predates these changes, capture its command and evidence separately; do not mark the feature complete while an introduced failure remains.

- [ ] **Step 2: Verify remote prerequisites without mutating CC Switch or the server**

In the operator shell, export the endpoint and token from the existing CC Switch `remote-vllm` Claude configuration into the child environment. Then run:

```bash
test -n "${FORGE_CUSTOM_ANTHROPIC_BASE_URL:-}"
test -n "${FORGE_CUSTOM_ANTHROPIC_API_KEY:-}"
FORGE_HEADLESS_PROVIDER=custom_anthropic FORGE_HEADLESS_MODEL=qwen3.6-35b-a3b-nvfp4-fast npm --prefix apps/desktop run eval:qwen:r1:anthropic -- --dry-run
```

Expected: exit 0 and secret-free dry-run output. Query the vLLM health and metrics URLs already used in the A/B smoke. Required pre-run state: health HTTP 200, `vllm:num_requests_running = 0`, `vllm:num_requests_waiting = 0`. If health is not 200 or waiting is nonzero and growing, stop; do not enqueue another case and do not restart automatically.

- [ ] **Step 3: Run remote R1 round 1 with exactly one client process**

```bash
FORGE_HEADLESS_PROVIDER=custom_anthropic \
FORGE_HEADLESS_MODEL=qwen3.6-35b-a3b-nvfp4-fast \
npm --prefix apps/desktop run eval:qwen:r1:anthropic -- \
  --output apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json
```

Expected: the runner executes `storage-validation`, `priority-labels`, and `due-date-labels` serially. Do not run an OpenAI-compatible comparison, a second Forge agent, or another remote evaluation while this process is active.

- [ ] **Step 4: Inspect round 1 before starting round 2**

```bash
jq '{success_rate: .report.success_rate, tasks: [.report.tasks[] | {task_id, passed, model_rounds, confirm_requests, scope_violations}]}' apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json
jq '[.traces[] | {task_id, failed_tool_count: (.raw_events | map(select(.event_type == "agent_turn_updated")) | last.state.failed_tool_count), changed_files, forbidden_files_changed, error, failure_reason}]' apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json
```

Expected hard checks:

- all three cases pass;
- no forbidden diff and no `package-lock.json` change;
- `storage-validation <= 10`, `priority-labels <= 14`, `due-date-labels <= 16` model rounds;
- any nonzero shell exit, `Unknown tool:`, or permission denial contributes to `failed_tool_count`;
- vLLM returns to health 200, running 0, waiting 0.

If a hard check fails, stop after round 1 and diagnose that trace. Do not hide a failed tool or widen permission policy to make the report green.

- [ ] **Step 5: Run and inspect remote R1 round 2**

Only after round 1 and remote health pass:

```bash
FORGE_HEADLESS_PROVIDER=custom_anthropic \
FORGE_HEADLESS_MODEL=qwen3.6-35b-a3b-nvfp4-fast \
npm --prefix apps/desktop run eval:qwen:r1:anthropic -- \
  --output apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-2.json
```

Repeat the exact `jq` and health checks from Step 4 against round 2. Expected: the same three hard passes and final running 0/waiting 0.

- [ ] **Step 6: Write the evidence report**

Create `docs/superpowers/reports/2026-07-17-anthropic-small-model-reliability-report.md` with this complete structure:

```markdown
# Anthropic Small-Model Reliability Report

Date: 2026-07-17
Model: qwen3.6-35b-a3b-nvfp4-fast
Transport: custom_anthropic
Concurrency: 1

## Baseline

| Case | OpenAI-compatible | Anthropic smoke |
|---|---:|---:|
| priority-labels | failed, 18 rounds, 43.361 s, 8 confirms | passed, 11 rounds, 29.034 s, 5 confirms |
| due-date-labels | model-round limit, 20 rounds, 50.232 s, 9 confirms | passed, 12 rounds, 38.116 s, 8 confirms |

## Local verification

List every command, exit code, and any pre-existing unrelated failure.

## R1 results

| Round | Case | Passed | Model rounds | Confirms | Failed tools | Forbidden diffs | Duration |
|---:|---|---|---:|---:|---:|---:|---:|

Populate all six rows directly from the two JSON artifacts.

## Runtime health

Record health status, running, and waiting before, between, and after rounds. Do not record endpoint credentials.

## Attribution

Separate protocol/adapter improvements from Agent runtime fixes. State whether any remaining failure is model behavior, Agent behavior, runner behavior, or infrastructure behavior, and cite the trace evidence.

## Decision

State whether the two-round hard gate passed. Recommend a separate convergence-controller design only if corrected traces still show model-round exhaustion or repeated no-progress verification.
```

- [ ] **Step 7: Refresh the GitNexus graph, then detect changed scope**

Run the repository-provided analyzer:

```bash
node .gitnexus/run.cjs analyze
```

Expected: the `forge` index advances to the implementation commit. If the command still fails because the local runner lacks `tree-sitter-swift`, record that exact infrastructure failure in the report, keep the existing graph available for read-only impact checks, and do not claim the graph was refreshed.

Then review the branch:

Run:

```bash
git status --short
git diff --stat main...HEAD
git diff --check main...HEAD
```

Run GitNexus `detect_changes(scope: "compare", base_ref: "main")`. Expected affected areas: eval script, Agent tool accounting, session setup, permission/shell policy, docs, and acceptance. Investigate any unrelated execution flow attributed to this branch before committing the report.

- [ ] **Step 8: Commit the report and sanitized artifacts**

Confirm neither artifact contains the endpoint token or authorization header:

```bash
rg -n "Authorization|Bearer |ANTHROPIC_AUTH_TOKEN|FORGE_CUSTOM_ANTHROPIC_API_KEY" docs/superpowers/reports/2026-07-17-anthropic-small-model-reliability-report.md apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-2.json
```

Expected: no matches. Then commit:

```bash
git add docs/superpowers/reports/2026-07-17-anthropic-small-model-reliability-report.md apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-1.json apps/desktop/artifacts/eval-runs/2026-07-17-anthropic-r1-round-2.json
git commit -m "test(eval): report anthropic qwen r1 reliability"
```

## Completion criteria

Implementation is complete only when:

- the named preset resolves `custom_anthropic` and the Qwen model without logging credentials;
- failed shell exits, unknown tools, permission denials, cumulative counts, traces, and loop progress agree;
- canonical in-workspace aliases are not external, while `..` and outward symlinks remain blocked;
- current-turn explicit protected files are session-scoped, reset on the next turn, and enforced before confirmation;
- protected `package-lock.json` blocks npm install/add/remove/update, while `npm test`, `npx tsc --noEmit`, and unprotected dependency changes retain existing behavior;
- local Rust, Node, formatting, Clippy, and acceptance gates pass;
- both serial Anthropic R1 rounds pass all three cases with no forbidden diff and healthy vLLM state;
- the final report attributes any remaining failures from trace evidence rather than assumption.
