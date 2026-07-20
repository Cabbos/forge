# Codex GPT–K3 Unified Routing and History Design

## Goal

Make the Codex desktop model picker show the current OpenAI GPT catalog and Kimi K3 at the same time, route each selected model to its real upstream, and keep the existing Codex task history visible when switching between GPT and K3.

## Current Evidence

- Codex currently has 561 unarchived threads with `model_provider = "openai"` and 3 K3 test threads with `model_provider = "custom"`.
- The 564 session files still exist. The apparent history loss is a GUI filter caused by changing the active provider identity, not data deletion.
- CCSwitch 3.17.0 projects only the current provider's model catalog into Codex.
- CCSwitch's proxy selects one current provider for the whole Codex application. It does not select an upstream provider from the request model.
- The current OpenAI cache contains eight model slugs: `codex-auto-review`, `gpt-5.3-codex-spark`, `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.5`, `gpt-5.6-luna`, `gpt-5.6-sol`, and `gpt-5.6-terra`.
- The Kimi route uses model ID `k3`, the Kimi Coding endpoint, honest Codex client identity, and CCSwitch's Responses-to-Chat-Completions conversion.

## Constraints

1. A cosmetic catalog merge is forbidden. Every displayed model must route to a working upstream.
2. OpenAI models continue using the existing ChatGPT/Codex login. No OpenAI API key is introduced.
3. K3 continues using the existing CCSwitch-managed Kimi credential. The credential must not be copied into project files, scripts, logs, or Codex live configuration.
4. The Codex live provider identity remains stable as `custom` so the GUI does not hide history when the selected model changes.
5. History migration may rewrite only the `session_meta.model_provider` field in rollout JSONL files, and only after a complete byte-for-byte backup. Conversation events, task content, model, and timestamps are never rewritten.
6. The router must preserve the real Codex User-Agent. It must not impersonate Claude Code.
7. Existing unrelated Forge worktree changes remain untouched. CCSwitch runtime work is developed and tested separately from Forge product source.

## Considered Approaches

### 1. Unified history with CCSwitch provider switching

Enable CCSwitch's built-in unified Codex history and keep using its one-click provider switch.

This is the lowest-risk option and fixes disappearing history, but GPT and K3 still do not appear together in one model picker. It does not satisfy the full goal.

### 2. Merge model names into the current Kimi catalog

Add GPT slugs to the Kimi model catalog.

This makes the names appear, but CCSwitch sends every request to Kimi. GPT selections would fail or be silently remapped. This approach is unsafe and rejected.

### 3. Model-aware CCSwitch routing

Keep one Codex-facing provider and add an explicit model-to-provider route table inside CCSwitch. The proxy chooses OpenAI Official for known OpenAI slugs and Kimi Bridge for `k3` before authentication, format conversion, and forwarding.

This is the selected approach. It reuses CCSwitch's existing official-login forwarding and Kimi protocol conversion instead of building a second credential-handling bridge.

## Architecture

Codex always connects to the CCSwitch loopback endpoint using one stable provider identity:

```text
Codex desktop GUI
  model_provider = custom
  model = selected slug
          |
          v
CCSwitch model-aware route table
  k3                       -> Kimi Bridge
  known OpenAI model slug  -> OpenAI Official
  unknown slug             -> reject locally
          |
          +-> Kimi Chat Completions conversion
          |
          +-> OpenAI Official Responses forwarding
```

The route decision happens before the current provider's endpoint, credential, reasoning conversion, or request override is applied. This prevents provider-specific settings from leaking across routes.

## Model Catalog

The Codex-facing catalog is a deterministic union of:

- the current official OpenAI entries from `/Users/cabbos/.codex/models_cache.json`; and
- the validated Kimi entry `k3`.

The union is deduplicated by slug. OpenAI entries retain their original display metadata, capabilities, and context values. The K3 entry uses the configured CCSwitch context value and reasoning capability metadata.

Catalog refresh must be atomic. If the OpenAI cache is missing or malformed, CCSwitch keeps the last known-good merged catalog rather than replacing it with a K3-only file.

## Provider Routing Rules

Routing is allowlist-based:

- `k3` routes only to `Kimi Bridge`.
- Slugs present in the captured OpenAI catalog route only to `OpenAI Official`.
- An unknown slug returns a local error naming the unsupported model; it never falls through to another provider.

Automatic failover does not cross provider families. A GPT request must not fall back to Kimi, and a K3 request must not fall back to OpenAI. Existing retry and circuit-breaker behavior remains available only within the selected route's provider.

## Authentication and Secret Boundaries

- OpenAI Official follows CCSwitch's existing ChatGPT-login proxy path and preserves the official Codex authentication state.
- Kimi follows the existing `Kimi Bridge` path and reads its credential from the CCSwitch database at request time.
- Codex receives only the existing local proxy placeholder credential.
- Request-body logging remains disabled.
- Logs may contain provider name, model slug, status, and timing, but never authorization headers or request bodies.

## Unified History

CCSwitch's unified Codex history mode is enabled only after a recoverable backup of:

- `/Users/cabbos/.codex/state_5.sqlite`;
- `/Users/cabbos/.codex/sessions/` metadata and rollout files;
- `/Users/cabbos/.codex/config.toml`;
- `/Users/cabbos/.codex/auth.json`; and
- `/Users/cabbos/.cc-switch/cc-switch.db`.

The migration normalizes OpenAI threads from `model_provider = "openai"` to the shared `custom` identity in both the state index and the rollout `session_meta` record. Existing K3 threads already use `custom`. It preserves each thread's model, title, timestamps, archive state, rollout path, conversation events, and content.

Before any rewrite, CCSwitch copies every affected rollout JSONL file and state database into its timestamped migration backup. It rewrites only the provider identity in the `session_meta` line, uses atomic replacement with a concurrent-change guard, and records a manifest so rollback can restore the original files and index exactly.

After migration, the GUI sees one history namespace. Selecting GPT or K3 no longer changes the provider identity, so records remain visible.

## Resuming Existing Threads

Each thread remains pinned to its stored model by default:

- an existing GPT thread resumes through OpenAI Official;
- a K3 thread resumes through Kimi Bridge.

Changing provider family inside an existing thread is blocked with a clear instruction to create a new task. This avoids sending provider-specific encrypted reasoning content to the wrong backend.

## CCSwitch Development and Installation Safety

The installed `/Applications/CC Switch.app` is not patched in place. The implementation is developed from the pinned CCSwitch source in a separate working directory and built as a side-by-side test application with its own bundle identity and test database.

Only after unit, integration, migration, rollback, and live GUI verification pass may the production application be replaced or the change be upstreamed. Replacing the production app is a separate confirmation-gated action.

## Error Handling

- Missing route target: reject before forwarding and name the missing CCSwitch provider.
- Unknown model: reject locally; do not guess or use the current provider.
- Kimi authentication or plan error: report Kimi status without switching to OpenAI.
- OpenAI login expiry: preserve Kimi availability and request normal ChatGPT reauthentication for GPT models.
- Catalog refresh failure: retain the last known-good merged catalog.
- History migration failure: roll back the database transaction and leave unified history disabled.
- Application crash during migration: recover from the backup manifest before allowing routing.

## Verification

1. Catalog test: all current OpenAI slugs plus `k3` appear exactly once.
2. Route unit tests: every allowed slug selects the expected provider; unknown slugs fail closed.
3. Credential isolation tests: GPT requests cannot read Kimi auth, and K3 requests cannot use ChatGPT auth.
4. Protocol tests: GPT remains Responses-native; K3 completes streaming and tool-call conversion.
5. History migration test: 561 OpenAI and 3 custom unarchived threads become visible in one namespace without changing titles, rollout paths, timestamps, or archive flags.
6. Resume tests: one existing GPT thread and one K3 thread resume through their original providers.
7. Cross-family test: changing a GPT thread to K3, or K3 to GPT, is blocked before network transmission.
8. Rollback test: restore original provider identities, catalogs, CCSwitch database, and Codex live files.
9. Secret scan: no upstream credential appears in logs, project files, scripts, model catalogs, or Codex live files.
10. GUI test: after one desktop restart, GPT models and K3 appear together and the existing history remains visible while switching selections.

## Rollout

1. Build and test the side-by-side CCSwitch variant against a disposable copy of the database and Codex home.
2. Run migration and routing verification against copied real metadata with credentials removed.
3. Back up production state.
4. Enable unified history and model-aware routing in a controlled production trial.
5. Verify GPT, K3, tool calls, history visibility, and provider-pinned resume behavior.
6. Keep the rollback bundle until the user has completed several normal sessions successfully.

## Non-Goals

- Combining GPT and K3 content into one backend conversation.
- Translating encrypted reasoning content between providers.
- Cross-provider automatic failover.
- Replacing ChatGPT login with a paid OpenAI API key.
- Spoofing client identity.
- Modifying Forge application behavior.
