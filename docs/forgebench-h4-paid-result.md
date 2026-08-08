# ForgeBench H4 Paid Holdout Result

This document records the first real sealed holdout campaign. It is a
diagnostic release result, not a passing frontier claim.

The campaign predates commit
`76d7822c6e4faf1ed402cc16c5b6c4e265c34d61`, which corrected model-round
accounting for parallel tool calls. The stored success, scope, trust, replay,
verification, and usage evidence remains immutable; the published round
metric is historical and must not be treated as authoritative for a current
release decision. A new source-pinned holdout run is required to measure the
round threshold after the fix.

## Locked execution

- Campaign: `h4-holdout-v1-control`
- Source commit: `9b1505f310dced122258cb95d1871aae1bcbfec6`
- Profile: `deepseek-v4-flash-anthropic-baseline-v1`
- Route: DeepSeek Anthropic Messages (`https://api.deepseek.com/anthropic`)
- Dataset fingerprint: `3be2ca93d66752275afb8f317fce7c04a479e30b22fbbfbf0afb887c0e7253e8`
- Harness fingerprint: `2fb6612000dc669ab1cf2e794fc9dc0f8831c195bd75230f31ababe63f4a2f66`
- Attempts: 60 (20 sealed cases × 3 repetitions)
- Total known cost: `1.80125860 CNY` of the locked `35 CNY` budget

The raw campaign directory is outside Git at
`/private/tmp/forge-h4-holdout-v1/control`. Its canonical report SHA-256 is
`391cd448ae6622c4e8b2990c83ecc456fe4c15914a638c1491a39ca2e563e9e2`, and its
artifact-index SHA-256 is
`afa195c7410e51cab4f7774e8b1983de1040ce53f4d1c4050c75412c67f98c65`.

## Gate result

| Metric | Observed | Threshold | Result |
| --- | ---: | ---: | --- |
| Trusted success rate | 0.7000 | ≥ 0.8000 | fail |
| Scope violation rate | 0.2333 (14/60) | ≤ 0.0200 | fail |
| Median model rounds | 31.5 | ≤ 12.0 | fail |
| Score coverage | 1.0000 | 1.0000 | pass |
| Harness contract coverage | 1.0000 | 1.0000 | pass |
| Workspace evidence | 1.0000 | 1.0000 | pass |
| Patch replay | 1.0000 | 1.0000 | pass |
| Verification evidence | 1.0000 | 1.0000 | pass |
| Usage known | 1.0000 | 1.0000 | pass |
| Terminal failure taxonomy | 1.0000 | 1.0000 | pass |

The canonical verifier returned exactly these blockers:
`holdout_model_rounds_above_threshold`,
`holdout_scope_rate_above_threshold`, and
`holdout_success_rate_below_threshold`. The campaign itself is structurally
valid (`60/60`, no pending records, known usage, and a verified hash chain);
the release decision is correctly `passed: false`.

There are two verification modes. During an active run, the default campaign
verification also requires the current Git commit and dirty-state digest to
match the frozen metadata. After this result was published, the documentation
changes necessarily changed the worktree digest; the sealed artifact remains
independently reproducible with:

```bash
cd apps/eval-runner
uv run python -m app.cli holdout verify \
  --campaign /private/tmp/forge-h4-holdout-v1/control \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json \
  --frozen
```

`--frozen` skips only the live repository commit/digest comparison. It still
checks the locked profile and dataset, every attempt identity, usage and trust
recomputation, budget hash chain, canonical report, artifact index, and the
same locked release thresholds. It never authorizes another provider call.

## Failure concentration

The largest case-level risks were:

- `forgebench-holdout-config-port-range`: 0/3 passed, 3/3 scope violations;
- `forgebench-holdout-integration-bulk-edit`: 0/3 passed, 3 verification failures,
  median 58 rounds;
- `forgebench-holdout-worker-quarantine`: 0/3 passed, 3 scope violations or
  verification failures;
- `forgebench-holdout-web-optimistic-rollback`: 0/3 passed, 2 verification
  failures and 1 scope violation;
- `forgebench-holdout-contract-task-forward-compat`: 1/3 passed, 2 verification
  failures.

The next harness iteration should target scope planning and bounded convergence
on these cases. It must return to the development split for offline changes and
new evidence before spending another holdout budget. This result does not
authorize threshold relaxation, case removal, hidden-verifier weakening, or a
frontier-quality claim.
