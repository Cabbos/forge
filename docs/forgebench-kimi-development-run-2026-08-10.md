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
