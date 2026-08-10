# Codex–Kimi Routing Design

Date: 2026-07-20
Status: Approved for implementation

## Goal

Make the existing Kimi provider stored in CCSwitch usable from local Codex through one-click provider switching. Preserve the current OpenAI provider as the fallback, start routing automatically after macOS login, and keep the upstream Kimi API key under CCSwitch management.

## Current State

- CCSwitch 3.17.0 is installed at `/Applications/CC Switch.app`.
- CCSwitch stores the active Kimi entry as a Claude/Anthropic provider.
- Kimi Code exposes an OpenAI-compatible Chat Completions endpoint at `https://api.kimi.com/coding/v1`.
- Local Codex custom providers use the OpenAI Responses API wire format.
- CCSwitch 3.17.0 includes Codex Responses-to-Chat-Completions conversion, streaming conversion, tool-call mapping, provider switching, and loopback routing at `127.0.0.1:15721`.

## Considered Approaches

### A. CCSwitch built-in Codex routing — selected

Create a Codex provider inside the installed CCSwitch and mark its upstream format as OpenAI Chat Completions. CCSwitch keeps Codex pointed at its local Responses endpoint and translates requests to Kimi. This reuses software already installed and responsible for provider state, avoids a second proxy, and keeps switching in one UI.

### B. Pinned external Node bridge

A separate zero-dependency bridge can perform the same translation, but it adds another background process, another local credential, separate logs, and a local fork to maintain. It is unnecessary because CCSwitch 3.17.0 already implements the required conversion.

### C. New custom bridge

A new adapter would maximize control but duplicate difficult protocol work such as SSE event ordering, custom tool calls, and multi-turn continuation. It is not justified while the installed CCSwitch implementation satisfies the requirement.

## Architecture

The request path is:

```text
Codex Responses API
  -> CCSwitch local routing at 127.0.0.1:15721/codex
  -> Responses-to-Chat-Completions conversion
  -> https://api.kimi.com/coding/v1/chat/completions
  -> CCSwitch converts streaming text and tool calls
  -> Codex Responses API events
```

CCSwitch remains the single source of truth for both the Kimi upstream credential and provider switching. No external bridge source tree, runtime, or updater is installed.

## Provider Configuration

Create a CCSwitch Codex provider named `Kimi Bridge` with:

- API key: copied internally from the existing CCSwitch Kimi provider
- Upstream Base URL: `https://api.kimi.com/coding/v1`
- Upstream API format: `openai_chat`
- Default model: `kimi-for-coding`
- Context window: `262144`
- Codex wire API: `responses`
- Model provider bucket: `custom`, matching CCSwitch's stable third-party history behavior
- Local routing required: enabled

The provider must not use the unrelated GLM fields stored alongside the existing Claude provider. Only the Kimi coding endpoint, Kimi credential, model ID, and relevant display metadata are copied.

## Credentials and Privacy

The Kimi API key stays inside the existing CCSwitch database and the provider state CCSwitch already manages. It is not written into documentation, scripts, shell profiles, logs, command-line arguments, or a new bridge directory.

Backups may contain the existing credential because they are full copies of the current CCSwitch/Codex state. Backup files must be stored under the existing user-private CCSwitch backup directory with restrictive permissions and must never be committed.

CCSwitch local routing listens only on `127.0.0.1`. Logging remains disabled or limited to metadata; request and response bodies and authorization headers must not be logged.

## Automatic Startup

CCSwitch must be registered as a per-user macOS login item so its local routing service becomes available after login without administrator privileges. The configuration must not create a second proxy LaunchAgent. If the app already owns a login-item registration, reuse it; otherwise create one entry whose only job is to open `/Applications/CC Switch.app` at user login.

## Switching Behavior

CCSwitch exposes both `OpenAI Official` and `Kimi Bridge` in its Codex provider list.

- Selecting `Kimi Bridge` starts or confirms CCSwitch local routing, preserves the official Codex login according to the installed CCSwitch behavior, and points live Codex configuration at the CCSwitch loopback endpoint.
- Selecting `OpenAI Official` restores the official provider configuration and leaves the saved Kimi provider available for later use.
- Codex must be restarted after switching because model-provider and model-catalog values are loaded at startup.

## Failure Handling

- Missing or ambiguous source Kimi provider: stop before creating the Codex provider.
- Empty Kimi key: stop without changing provider state.
- CCSwitch not running: Codex receives a clear loopback connection failure; launching CCSwitch restores service.
- Port `15721` occupied by another process: stop and report the owning process before enabling takeover.
- Upstream authentication, entitlement, or quota failure: preserve the upstream status and message without logging credentials.
- Failed switch or validation: restore the backed-up CCSwitch database, `~/.codex/config.toml`, and `~/.codex/auth.json`.

## Verification

Implementation is complete only when all of the following pass:

1. Backups exist and have user-only permissions.
2. The new CCSwitch Codex provider contains the Kimi endpoint, `openai_chat` API format, `kimi-for-coding` model, and 262144-token context declaration.
3. No new file outside the approved backups contains the Kimi API key.
4. CCSwitch local routing listens only on `127.0.0.1:15721`.
5. Unauthenticated or invalid local routing requests are rejected according to CCSwitch's managed token behavior.
6. Switching to `Kimi Bridge` updates the live Codex provider without deleting the saved OpenAI provider.
7. A simple Codex prompt streams a valid Kimi response.
8. A tool-call round trip succeeds, including tool name, call ID, JSON arguments, and tool output.
9. Switching back to `OpenAI Official` restores official Codex operation.
10. CCSwitch starts after a controlled login-item launch and makes local routing healthy.

## Rollback

Rollback consists of switching to `OpenAI Official`, disabling Codex local routing if it is no longer used, removing the `Kimi Bridge` Codex provider, removing only the login item created by this implementation, and restoring the pre-change database/config/auth backups if necessary. The original Claude-side Kimi provider remains untouched.
