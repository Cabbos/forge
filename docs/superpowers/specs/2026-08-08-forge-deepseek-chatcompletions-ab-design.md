# Forge DeepSeek Chat Completions A/B Design

> Date: 2026-08-08
> Status: Approved under the standing 100 CNY harness authorization
> Scope: Phase H2 only
> Parent design: `2026-08-07-forge-frontier-deepseek-harness-design.md`

## Outcome

Phase H2 will compare the two transports that DeepSeek officially supports for
DeepSeek V4 Flash:

- Anthropic Messages at `https://api.deepseek.com/anthropic`;
- OpenAI Chat Completions at `https://api.deepseek.com`.

The phase will not implement or claim OpenAI Responses support. DeepSeek's
2026-04-24 change log and current model documentation list Chat Completions and
Anthropic as the supported V4 API families, but do not list Responses.

The A/B uses the same model, development cases, task order, repetitions,
budgets, tool catalog, prompt mode, context mode, verification policy, source
commit, and controller. Only the transport and endpoint family differ.

## Selected architecture

The eval controller derives two artifacts from the same canonical harness
profile:

- a typed expected provider route in the `forge_eval_agent` request;
- a secret-free one-provider settings file at the runner's isolated
  `<runtime_home>/.forge/config.json`.

The child process already receives that isolated runtime home as `HOME` and
`USERPROFILE`. The settings file therefore exists only inside the attempt
runtime and never changes desktop settings or the user's
`~/.forge/config.json`.

For the first version, the Rust boundary accepts exactly these two routes:

| Provider | Model | Transport | Base URL |
|---|---|---|---|
| `deepseek` | `deepseek-v4-flash` | `anthropic_messages` | `https://api.deepseek.com/anthropic` |
| `deepseek` | `deepseek-v4-flash` | `openai_chat_completions` | `https://api.deepseek.com` |

Any other expected route fails before session creation and before a provider
call. Rust loads the effective `deepseek` profile through the existing settings
loader, then requires its provider, model, transport, and normalized base URL
to equal the typed expected route. The existing credential resolver and
session builder subsequently read that same isolated settings file through
their normal paths.

No shared session-builder request, function signature, or desktop provider
default changes. This deliberately avoids the HIGH/CRITICAL shared session
construction surface while still making Rust attest the effective route before
use.

The existing `AnthropicAdapter` remains the control arm. The existing
`OpenAiCompatibleAdapter` becomes the candidate arm; no provider-specific
branches are added to its stream parser unless an offline DeepSeek fixture
proves a protocol difference.

## Request and evidence contract

The headless request adds:

```json
{
  "provider_route": {
    "transport": "openai_chat_completions",
    "base_url": "https://api.deepseek.com"
  }
}
```

The controller derives this object and the isolated settings file from a
validated, canonical harness profile. The settings file contains environment
variable names such as `DEEPSEEK_API_KEY`, never the secret value. Callers
cannot supply an arbitrary URL through a paid campaign profile, and Rust does
not trust the settings file without comparing it to the typed expected route.
The original H1 Anthropic profile intentionally omits `base_url` for backwards
compatibility; its `transport` plus `endpoint_family` resolve to the same
official URL, so old locked profile bytes and fingerprints remain valid. New
candidate profiles carry the explicit OpenAI base URL.

Credential preflight validates provider, model, transport, endpoint, and API
key availability without starting a provider call. A run artifact records the
transport and endpoint family in its frozen profile and records the transport
again in the `eval_agent_identity` event. Independent campaign verification
recomputes the profile fingerprint and rejects mismatched identity.

OpenAI usage uses the provider's inclusive `prompt_tokens` semantics:
`cache_miss = input - cache_read`. Anthropic usage keeps the additive
`input + cache_creation + cache_read` semantics. Missing or contradictory
usage remains a blocker.

## Experiment profiles

The current `HarnessProfileV1` becomes a constrained multi-profile schema. It
continues to require:

- provider `deepseek`;
- model `deepseek-v4-flash`;
- one of the two route pairs in the table above;
- development split, 10 cases, three repetitions, and 30 attempts;
- canonical JSON and a secret-free official CNY price snapshot;
- per-task, campaign, and global budget hierarchy.

Two committed profiles define the paired arms:

- `deepseek-v4-flash-anthropic-baseline-v1`;
- `deepseek-v4-flash-openai-chat-control-v1`.

All non-transport dimensions match exactly. Comparison fails closed if more
than `transport`, `endpoint_family`, `profile_id`, and the resulting fingerprint
differ.

## Offline conformance

Before any paid candidate call, tests must prove:

- route override rejects a wrong provider, model, transport, endpoint, and
  non-HTTPS URL;
- credential preflight reports the selected transport and starts no call;
- the isolated effective profile selects `OpenAiCompatibleAdapter` only for
  the candidate while the ordinary desktop DeepSeek default stays Anthropic;
- Chat Completions request bodies preserve Forge tool definitions;
- streaming text, one tool call, parallel tool calls, malformed arguments,
  usage, provider error, keep-alive comments, cancellation, and truncated
  streams have deterministic fixtures;
- tool call ids remain adjacent to tool results across rounds;
- canonical usage reaches the Python budget ledger with known CNY cost;
- the built-in desktop DeepSeek route remains Anthropic.

No paid call occurs until the Rust adapter, headless, Python profile, baseline,
and acceptance tests pass.

## Paid comparison

The candidate campaign starts with one paid smoke. It stops on provider schema
drift, unknown usage, untrusted workspace evidence, patch replay failure, or
route identity mismatch.

If the smoke is trusted, the controller runs the remaining candidate attempts.
The immutable Anthropic baseline and candidate campaign are compared by task
and repetition. The report includes trusted success, verification, scope,
median and p95 model rounds, cost, duration, provider errors, and trust
blockers.

H2 selects a winner only when the candidate has a higher trusted success rate,
or equal success with a material improvement in rounds, scope, cost, or
latency. Safety and trust cannot regress. If neither arm materially wins, the
Anthropic route remains the default H3 arm.

## Error and recovery boundary

- Invalid route: terminal preflight failure, zero provider calls.
- Interrupted paid call: preserve the pending marker and never replay it.
- Unknown usage: reserve the per-task ceiling and block the campaign.
- Untrusted attempt: publish the immutable attempt and stop.
- Provider error: classify it without rewriting the attempt.
- Source or profile change: require a new campaign id and fingerprint.

## Documentation and claim boundary

The parent frontier design is corrected to replace “native Responses” with
“official transport A/B”. Product documentation must state that Forge supports
OpenAI-compatible Chat Completions for this experiment, not OpenAI Responses.

Passing H2 proves protocol conformance and identifies the better supported
DeepSeek transport on the development split. It is not a holdout result or a
frontier-grade claim.
