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

## Next gate

The offline H3 work should diagnose or improve the bulk-update execution path,
then rerun the complete development pair only after the acceptance gate passes.
The H4 holdout remains closed.
