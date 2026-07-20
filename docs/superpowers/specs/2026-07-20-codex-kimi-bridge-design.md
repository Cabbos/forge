# Codex–Kimi Bridge Design

Date: 2026-07-20
Status: Approved for implementation

## Goal

Make the existing Kimi provider stored in CCSwitch usable from local Codex through one-click provider switching. Preserve the current OpenAI provider as the default/fallback, start the compatibility bridge automatically at macOS login, and avoid copying the upstream Kimi API key into any new plaintext file.

## Current Constraints

- CCSwitch stores the current Kimi entry as a Claude/Anthropic provider.
- Kimi Code exposes Anthropic Messages and OpenAI-compatible Chat Completions endpoints.
- Local Codex custom providers use the OpenAI Responses API wire format.
- Kimi Code's `/v1/responses` endpoint is unavailable, so direct Codex configuration cannot work.
- The existing `~/.codex/config.toml` and OpenAI authentication must remain recoverable through CCSwitch.

## Considered Approaches

### A. Pinned zero-dependency Node bridge — selected

Install an audited, pinned revision of a small Responses-to-Chat-Completions bridge and add a narrow Kimi provider adaptation. This gives Codex-compatible streaming, tool calls, and multi-turn continuation without installing a large dependency graph. Pinning prevents unreviewed automatic upgrades.

### B. LiteLLM

LiteLLM provides a general-purpose local gateway, but it introduces a large dependency surface and has had compatibility issues when converting Codex custom tools from Responses API to Chat Completions. It is not selected for this local, single-provider use case.

### C. New custom bridge

A bridge written from scratch would maximize control but duplicate difficult protocol work such as SSE event ordering, tool-call round trips, reasoning fields, and `previous_response_id` continuation. It is not selected unless the audited bridge proves unsuitable.

## Architecture

The bridge is installed outside the Forge repository at:

```text
~/.local/share/codex-kimi-bridge/
```

A macOS LaunchAgent starts it at login and restarts it after an unexpected exit. The service listens only on:

```text
http://127.0.0.1:4057
```

The request path is:

```text
Codex Responses API
  -> local authenticated bridge
  -> Kimi Chat Completions API
  -> local bridge converts streaming events/tool calls
  -> Codex Responses API events
```

The implementation adds a CCSwitch Codex provider named `Kimi Bridge`. CCSwitch remains responsible for switching between the existing OpenAI provider and the local Kimi bridge.

## Credentials

The existing upstream Kimi key remains owned by CCSwitch. The LaunchAgent starts through a wrapper that reads the key from `~/.cc-switch/cc-switch.db` and exports it only to the bridge process. The wrapper must fail closed if the provider is missing, duplicated ambiguously, or has an empty key.

The bridge uses a separate randomly generated local access token for inbound Codex requests. This token is not an upstream credential. CCSwitch stores it as the API key for the `Kimi Bridge` Codex provider so Codex can authenticate to the loopback service.

The design does not copy the Kimi key into the bridge directory, LaunchAgent plist, Codex config, shell startup files, logs, or command-line arguments.

## Configuration

The CCSwitch Codex provider uses:

- Name: `Kimi Bridge`
- Base URL: `http://127.0.0.1:4057/v1`
- Model: `kimi-for-coding`
- Wire API: Responses
- API key: generated local bridge token

The existing OpenAI provider and its stored configuration are left intact. Switching back through CCSwitch restores the previous OpenAI configuration.

## Bridge Behavior

The bridge must:

- translate Responses API input into Kimi's OpenAI-compatible Chat Completions format;
- translate streaming text and completion state back into valid Responses API SSE events;
- preserve function/tool names, JSON arguments, tool-call IDs, and tool outputs across turns;
- retain bounded `previous_response_id` state for multi-turn continuation;
- map unsupported reasoning-effort values conservatively;
- pass through Kimi model IDs without presenting them as OpenAI models;
- preserve an honest client identity and avoid impersonating another supported client;
- reject non-loopback binding and unauthenticated requests;
- avoid logging prompts, responses, authorization headers, or secrets.

No image, audio, web-search, or provider-specific features are added unless they are already supported safely by the selected bridge.

## Automatic Startup

Create a user LaunchAgent under:

```text
~/Library/LaunchAgents/
```

It runs the credential-loading wrapper, binds the bridge to loopback, writes minimal operational logs to a user-local state directory, and restarts on failure. The service must not require administrator privileges.

## Failure Handling

- Missing CCSwitch database or Kimi provider: exit with a concise non-secret error.
- Missing/empty Kimi key: exit without starting the listener.
- Port already in use: exit and record the port conflict.
- Upstream authentication failure: return the upstream status without logging the credential.
- Unsupported request/tool shape: return an explicit compatibility error instead of silently dropping tools.
- Bridge crash: LaunchAgent restarts it with normal macOS backoff behavior.

## Verification

Implementation is complete only when all of the following pass:

1. Static review confirms the pinned bridge contains no unexpected credential exfiltration, telemetry, auto-update, or non-Kimi outbound destinations.
2. Bridge unit/smoke tests pass at the pinned revision.
3. `/health` succeeds on loopback and the service is unreachable through non-loopback interfaces.
4. Requests without the local token are rejected.
5. A simple Codex prompt streams a valid Kimi response.
6. A tool-call round trip succeeds, including tool-call ID and JSON argument preservation.
7. A second turn succeeds through `previous_response_id` continuation.
8. CCSwitch can switch to `Kimi Bridge` and back to the existing OpenAI provider without losing either configuration.
9. The bridge starts again after logout/login or a controlled process termination.
10. A secret scan confirms that the Kimi key was not added to new files or logs.

## Rollback

Rollback consists of switching CCSwitch back to the existing OpenAI provider, unloading and deleting the Kimi LaunchAgent, removing the `Kimi Bridge` CCSwitch Codex provider, and deleting `~/.local/share/codex-kimi-bridge/` plus its non-secret logs. The original CCSwitch Kimi provider and current Codex OpenAI configuration remain untouched.
