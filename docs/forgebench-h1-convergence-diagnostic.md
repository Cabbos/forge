# ForgeBench H1 convergence-guard diagnostic

This is a development-only, source-pinned before/after diagnostic for the
advisory prompt guard added in commit `cacfe290ec16b41d54d7818b84985d9878bf267f`.
It is not a paired causal A/B comparison: the two 30-attempt campaigns ran from
different source commits. No holdout was opened, and no frontier-quality claim
is made.

## Locked evidence

- Dataset fingerprint: `39ac183a1f3f8e7c0f29dbbe5d4e302765935d94417f8c2adffb984632223dbd`;
- Profile: `deepseek-v4-flash-anthropic-baseline-v1`;
- Harness fingerprint: `2fb6612000dc669ab1cf2e794fc9dc0f8831c195bd75230f31ababe63f4a2f66`;
- Before campaign: `h1-anthropic-smoke-h0`, source `1a2e0b9d4288a1b79cc9f4155726eb5cb2609518`;
- After campaign: `h1-convergence-cacfe`, source `cacfe290ec16b41d54d7818b84985d9878bf267f`;
- Raw campaign roots remain outside Git at `/private/tmp/forge-h1-smoke-h0` and
  `/private/tmp/forge-h1-convergence-cacfe`.

The secret-free compact record is
[`h1-convergence-cacfe-summary.json`](../apps/eval-runner/forgebench/results/h1-convergence-cacfe-summary.json).
It records the canonical report and artifact-index SHA-256 values without raw
provider text, credentials, or attempt transcripts.

## Metrics

| Metric | Before | After | Direction |
| --- | ---: | ---: | --- |
| Trusted success | 0.8667 | 0.8667 | unchanged |
| Verification pass | 0.9667 | 1.0000 | improved |
| Scope violations | 0.1000 | 0.1333 | regressed |
| Median model rounds | 16.5 | 14.0 | improved |
| P95 model rounds | 32.0 | 36.0 | regressed |
| Median tool calls | 26.0 | 21.5 | improved |
| P95 tool calls | 50.0 | 56.0 | regressed |
| Repeated tool-call rate | 0.7884 | 0.7651 | improved |
| Median cost (CNY) | 0.01902320 | 0.01716026 | improved |
| Total cost (CNY) | 0.71910864 | 0.71606912 | improved |

The guard appears to reduce typical-loop waste, but the scope and tail
regressions are material. It therefore remains an advisory development
hypothesis and is **not promoted as a winner**. The next harness iteration
should strengthen the scope firewall and make read-only boundaries more
explicit before spending another holdout budget.
