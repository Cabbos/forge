# ForgeBench DeepSeek H2 Official Transport A/B

H2 is the transport harness experiment after the H1 Anthropic control. It uses
the same DeepSeek V4 Flash model, frozen ForgeBench development split, task
order, repetitions, budgets, tool catalog, verification policy, source commit,
and controller in both arms. Only the official transport and endpoint family
change.

## Allowed routes

The profile schema and Rust headless boundary accept exactly these route tuples:

| Arm | Provider | Model | Transport | Base URL |
| --- | --- | --- | --- | --- |
| Anthropic control | `deepseek` | `deepseek-v4-flash` | `anthropic_messages` | `https://api.deepseek.com/anthropic` |
| OpenAI candidate | `deepseek` | `deepseek-v4-flash` | `openai_chat_completions` | `https://api.deepseek.com` |

These are an Anthropic Messages route and an OpenAI-compatible Chat Completions
route. H2 does not implement or claim OpenAI Responses support for DeepSeek.
Any other provider, model, transport, endpoint, or non-HTTPS route fails closed
before session creation and before a provider call.

## Eval-only isolation

The controller derives a typed expected route from a canonical profile and writes
one secret-free provider profile under the child process's isolated
`<runtime_home>/.forge/config.json`. The API key is represented only by its
environment-variable name. Rust loads that effective profile through the normal
settings path and requires an exact provider/model/transport/base-URL match
before preflight or session creation. This is an eval-only isolated route
override, not a desktop settings migration or shared session-builder change.
The ordinary desktop DeepSeek default remains Anthropic.

## Trust and spend boundary

Both arms use the H1 pending marker, immutable attempt artifacts, usage ledger,
budget caps, workspace observer, clean-fixture patch replay, and independent
verification. A paid campaign stops without retry when it observes:

- route identity mismatch or profile/source fingerprint drift;
- provider schema drift or provider error;
- unknown, malformed, contradictory, or over-cap usage/cost;
- untrusted workspace evidence, scope violation, or patch replay failure;
- an interrupted paid call whose exact charge cannot be known.

Recovery never reissues an interrupted pending call. Known usage is priced using
the official CNY snapshot; Anthropic cache fields retain their additive
semantics, while Chat Completions uses inclusive `prompt_tokens` semantics.
When a controller-side interruption is observed after the durable pending marker,
recovery preserves only a bounded non-secret stage marker and emits it as a
`baseline_interruption` raw event; the exact charge remains unknown and the arm
stays blocked.

## Offline gate and claims

Run the offline contract before any paid candidate call:

```bash
scripts/acceptance.sh --only "DeepSeek H2 official transport A/B contract"
```

The gate covers Rust route validation and adapter conformance plus Python
profile, runner, comparison, and usage contracts. Passing H2 proves the paired
transport harness is wired and fail-closed on the development split. It does
not prove a holdout score, frontier KPI, or production certification.

The comparison artifact must show both arms are trusted and complete, verify
that transport/endpoint family is the only experimental difference, and publish
per-task success, scope, rounds, cost, latency, provider errors, trust blockers,
and the selected arm. If the candidate does not materially improve trusted
success or safety-preserving efficiency, Anthropic remains the default for the
next phase.
