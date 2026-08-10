# ForgeBench Kimi K3 development run — 2026-08-10

This is a reproducible, development-only diagnostic for the Forge-built
evaluation harness. It is not a holdout result, a frontier-quality claim, or a
claim about the underlying model independent of the harness.

## Run identity

- Source commit: `32b8fdeb` (`feat(eval): fail closed on unknown cost`)
- Dataset: ForgeBench `development`, 10 cases
- Provider chain: Kimi `k3` through the typed OpenAI-compatible coding route;
  DeepSeek V4 Flash fallback was enabled but not used
- Cost gate: `--require-known-cost --max-total-cost-usd 5.00`
- Output: report-only artifact with raw traces kept outside the repository
- Credential boundary: `FORGE_CUSTOM_OPENAI_API_KEY` and `DEEPSEEK_API_KEY`
  were inherited by the runner process; no values are present in this document
  or the committed source tree

The command shape was:

```bash
uv run python -m app.cli \
  --cases forgebench/cases/development \
  --provider forge --provider-chain kimi-k3-deepseek --model k3 \
  --report-only --require-known-cost --max-total-cost-usd 5.00 \
  --output /tmp/forge-kimi-full-v2.json
```

## Aggregate evidence

| Metric | Result |
| --- | ---: |
| Tasks | 10 |
| Success rate | 1.0000 |
| Verification pass rate | 1.0000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 11.4 |
| Input tokens | 391,410 |
| Output tokens | 21,326 |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Total Kimi K3 cost | `$0.6010896` |
| Fallback events | 0 |
| Pricing snapshot events | 10 |

Per-case model rounds were `23, 9, 9, 11, 12, 5, 11, 12, 13, 9` in the
canonical development case order. The run therefore demonstrates that the
route identity, isolated runtime, independent verification, scope observer,
provider usage evidence, frozen Kimi price snapshot, and unknown-cost gate can
close together in one real campaign.

Kimi K3 prices are frozen from the official source recorded by the harness:
<https://platform.kimi.ai/docs/pricing/chat-k3>. Missing cache-hit metadata is
priced conservatively as cache-miss input; malformed or incomplete usage would
remain unknown and fail the explicit cost gate.

## Claim boundary

This run did not open the sealed holdout, compare a paired control/candidate
arm, or execute a calibrated red-team lane. It is strong development evidence
for harness execution and accounting, not evidence that Forge has passed the
H4 release thresholds or is frontier-grade.

## Follow-up after prompt and process-boundary fixes

The earlier run above is retained as historical evidence. After commit
`54672b81` mirrored the controller prompt into Rust's task-first prompt field,
and `a737d3de` fixed long stdin payloads in the bounded subprocess runner, the
same 10 development cases were rerun with the same Kimi route and a 12-round
cap. The report artifact was kept outside Git at
`/tmp/forge-kimi-ccswitch-post-stdin-fix-10.json`.

| Metric | Follow-up result |
| --- | ---: |
| Tasks | 10 |
| Success rate | 0.9000 |
| Verification pass rate | 0.9000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 4.1 |
| Input tokens | 109,096 |
| Output tokens | 7,969 |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Total Kimi K3 cost | `$0.2436102` |
| Trust result | `trusted` |

The one rejected task passed its visible target validation but failed the
controller-owned full monorepo regression. The new bounded
`eval_verification_ladder` trace event records that distinction without
exposing hidden verifier commands. The failure is retained as a non-inferiority
signal, not silently counted as success.

A separate one-case boundary check forced an invalid Kimi credential and
verified a real DeepSeek V4 Flash fallback: success `1.0`, verification `1.0`,
scope `0.0`, known cost `$0.000798`, and an `authentication` fallback event.
Both provider credentials were read in memory from the local CCSwitch store;
no key is present in this document or the repository.

## Follow-up after controller-owned repair reopening

After the Rust controller learned to reopen the bounded write boundary for one
controller-owned repair turn, the development set was rerun with the same Kimi
K3 route, a 12-round cap, and the same isolated full-verification command. The
report artifact is outside Git at
`/tmp/forge-kimi-ccswitch-controller-repair-open-10.json`.

| Metric | Follow-up result |
| --- | ---: |
| Tasks | 10 |
| Success rate | 0.9000 |
| Verification pass rate | 0.9000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 4.5 |
| Input tokens | 151,998 |
| Output tokens | 10,784 |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Total Kimi K3 cost | `$0.3108612` |
| Trust result | `trusted` |

Nine cases completed with full verification. The remaining admin bulk-status
case exhausted its 12 model rounds before producing a converged result; it did
not violate scope. This is an explicit budget signal, not a hidden success.
The new controller repair path is covered by focused tests and the same report
now records the verification ladder and its first rejecting stage.

The scorer correction landed after this artifact was produced: the report's
stored aggregate had marked prepared-turn evidence `0/10` because it compared
the rendered controller prompt to the raw task string byte-for-byte. Offline
re-scoring the unchanged ten traces with the corrected prefix-aware rule gives
`10/10` prepared-turn evidence; task outcomes, verification, scope, usage, and
cost remain unchanged.

## Follow-up: case budget and safety-boundary diagnostic

The admin multi-file case was then run without the exploratory 12-round CLI
cap, allowing its declared 40-round case budget to reach the agent. It passed
targeted, hidden, and full verification in 10 model rounds with zero scope
violations and known Kimi cost `$0.1061568`. The trace attempted a `git log`
future-state lookup, however, so the corrected trust gate classifies this
evidence as `untrusted` despite the functional pass; it is not a quality claim.

A second attempt with the explicit history/refs prohibition encountered a
provider stream decoding error. The controller had previously allowed each of
five verification phases to consume its own 300-second timeout, producing an
invalid ~42-minute wall time. The task-level monotonic deadline fix now bounds
that entire lifecycle and records the timeout as a failed trace rather than
letting validation multiply the case budget.

## Latest full development rerun after the deadline and trust fixes

The latest harness was rerun across all ten development cases without the
exploratory 12-round CLI cap. The report and raw traces are outside Git at
`/tmp/forge-kimi-ccswitch-latest-no-cap-10.json`.

| Metric | Latest result |
| --- | ---: |
| Tasks | 10 |
| Success rate | 1.0000 |
| Verification pass rate | 1.0000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 4.1 |
| Average repair attempts | 0.1 |
| Input tokens | 128,406 |
| Output tokens | 8,457 |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Total Kimi K3 cost | `$0.2625498` |
| Trust result | `trusted` |
| Fallback events | 0 |

All universal safety and evidence scores are `1.0`, including future-state
leakage, prepared-turn evidence, and verification evidence. The admin
multi-file case completed in 9 rounds with one bounded controller repair and
three validation attempts. This is the strongest current development result,
but it remains development-only and does not unlock the sealed holdout.

## Current rerun after provenance identity fix

On 2026-08-10, commit `3005ba41` reran the same ten development cases with the
CCSwitch Kimi K3 credential and the explicit `kimi-k3-deepseek` chain. The
fallback was armed but not needed: all ten execution-identity events resolved
to `custom_openai/k3` at `https://api.kimi.com/coding/v1`. The report is kept
outside Git at `/tmp/forge-kimi-k3-deepseek-current-20260810.json`; its SHA-256
is `38d8bca3b28419ccbef137aac83a987ab64c525de5895c817ff1f3abd4986ba0`.

| Metric | Current result |
| --- | ---: |
| Tasks | 10 |
| Success rate | 1.0000 |
| Verification pass rate | 1.0000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 4.6 |
| Input tokens | 106,086 |
| Output tokens | 8,369 |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Total Kimi K3 cost | `$0.2412714` |
| Fallback events | 0 |
| Trust result | `trusted` |

The provenance fix makes `AgentTrace.model` carry the exact provider model
(`k3`) while the versioned harness profile remains a separate identity. This
closes the trace-to-campaign identity check; it is a harness correctness
improvement, not evidence of a model-quality or holdout improvement.
