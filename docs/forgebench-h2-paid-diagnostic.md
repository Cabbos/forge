# ForgeBench H2 Paid Diagnostic

This is a development diagnostic record for the H2 transport harness. It is
not a benchmark result, holdout result, or frontier claim.

## Frozen inputs

- H2 implementation source: `5eb5bde948bbb6be6bcf5e2adc0974dabbb68e3a`
- Dataset fingerprint: `39ac183a1f3f8e7c0f29dbbe5d4e302765935d94417f8c2adffb984632223dbd`
- Anthropic control profile fingerprint: `2fb6612000dc669ab1cf2e794fc9dc0f8831c195bd75230f31ababe63f4a2f66`
- OpenAI Chat Completions candidate fingerprint: `3da2db5467faf01523dd7df0df1d02048b0caffa6ce632b9f4c2ed501917ef03`

The two smoke calls were real Forge headless calls after the zero-call typed
preflight:

| Arm | Route | Smoke cost (CNY) | Verify |
| --- | --- | ---: | --- |
| H1 control | Anthropic Messages | `0.0333108` | `ok: true` |
| H2 candidate | OpenAI Chat Completions | `0.0532278` | `ok: true` |

After the bounded command-string fix, a fresh isolated H2 candidate smoke also
completed one paid development attempt from source commit
`7dba8291ddd639ef5267b86a3aab9d00ccbacded`. The attempt was trusted, passed
verification and scope checks, recorded known usage, and cost `0.05115632 CNY`.
It is retained as a route-path diagnostic only; it does not extend either
historical seven-attempt campaign, complete a 30-attempt arm, or establish an
A/B winner.

The controller also persists the initial `runner` stage in the pending marker.
This makes an external process interruption auditable without guessing whether
usage, trust evaluation, or artifact publication had started; recovery still
reserves the full per-task ceiling and never replays the call.

## Fail-closed outcome

Five diagnostic campaigns were allowed to run only until the first unknown-cost
interruption on the same development case, `forgebench-dev-task-api-bulk-update`:

| Campaign family | Terminal attempts | Final status | Final reserved spend (CNY) |
| --- | ---: | --- | ---: |
| H1 Anthropic control (`v4`, `v5`, `v6`) | 7 each | blocked | `1.16291260`, `1.13841944`, `1.16829568` |
| H2 OpenAI candidate (`v1`, `v2`) | 7 each | blocked | `1.12941768`, `1.16590420` |

The final attempt in each campaign had an unknown charge. Recovery reserved the
per-task ceiling, emitted a bounded `baseline_interruption` stage event, removed
the pending marker, and never reissued the call. Combined reserved spend across
these diagnostics was `5.76494960` CNY, below the 100 CNY program ceiling.

Because neither arm reached 30 complete trusted attempts, no paired comparison
artifact was published. The correct conclusion is that the harness successfully
proved both official routes and stopped safely at an unresolved provider/model
execution boundary; it did not establish a transport winner.

## Latest synchronized-source diagnostics

After the interruption-stage hardening, a clean paid-run worktree produced the
following additional evidence from one shared source commit
`cf7e4d4d31690db734ae59a84888405231699e4f`:

| Campaign | Terminal attempts | Final status | Spend (CNY) | Boundary |
| --- | ---: | --- | ---: | --- |
| `h2-control-v3` | 4 | blocked | `0.11575648` | `patch_replay_failed` / untrusted |
| `h2-candidate-v3` | 1 | active | `0.03946696` | trusted smoke only |

The candidate smoke is a valid route-path artifact, but the control stopped on
an independent trust failure before a paired arm existed. These artifacts are
diagnostic evidence, not a comparison result; the controller did not retry the
failed control attempt.

After the cache-boundary fix, a new source-pinned pair (`bda5d977b85d234ffa3588f355302f2eb5cf3bc3`) produced six trusted, known-cost attempts on each arm before the same multi-file API case reached an unknown-cost runner interruption:

| Campaign | Trusted attempts | Terminal attempts | Final status | Reserved spend (CNY) |
| --- | ---: | ---: | --- | ---: |
| `h2-control-v4` | 6 | 7 | blocked | `1.13673496` |
| `h2-candidate-v4` | 6 | 7 | blocked | `1.18498476` |

Both arms passed the cache isolation contract, scope, verification, and patch
replay on all six trusted attempts. Recovery reserved the one-CNY per-task
ceiling for the interrupted attempt and did not replay it. This is stronger
route and harness evidence, but still not a complete paired 30-attempt result.

## Artifact-publication root cause and fix

The v4 interruption was independently reproduced offline after the paid
artifacts were frozen. The Forge runner completed a valid trace with known
usage, but `BaselineAttemptArtifact` rejected its verification metadata because
the trace contained ordinary source code such as
`api_key = os.getenv('DEEPSEEK_API_KEY')`. The old secret scrub pattern treated
the identifier assignment itself as a secret-like value, so the controller
collapsed a publishable attempt into the fail-closed unknown-cost boundary.

The harness now matches only token-shaped `sk-...` and Bearer values. A
regression test proves environment-variable lookup source is accepted while
the existing token rejection remains. This corrects a false positive without
weakening secret-material rejection. The v4 ledgers and raw artifacts remain
immutable diagnostic evidence; they were not replayed or rewritten.

## Cache-boundary hardening

The `cf7e4d4d` control diagnostic exposed a real harness boundary: the model
overrode the inherited Bun/uv cache variables and wrote `.bun-cache/` inside
the observed workspace. That attempt was correctly stopped as untrusted
(`patch_replay_failed`) rather than replayed. The follow-up harness change now
routes `XDG_CACHE_HOME`, `BUN_INSTALL_CACHE_DIR`, and `UV_CACHE_DIR` into the
isolated `runtime-state`, and the Python/Rust observers share a narrow,
explicit `.bun-cache` disposable-directory allowlist. Product files beside
that directory remain observed, and a model-reported cache path still creates
a report/observation mismatch. This fix is offline-tested; the old paid
artifact remains immutable diagnostic evidence and is not replayed. Recovery
also records only the bounded exception class alongside the interruption stage,
never exception/provider text.

## Next gate

The offline H3 work should diagnose or improve the bulk-update execution path,
then rerun the complete development pair only after the acceptance gate passes.
The H4 holdout remains closed.
