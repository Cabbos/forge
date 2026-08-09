# Kimi K3 / DeepSeek paired diagnostic — 2026-08-10

The eval runner now has a route-pure provider A/B diagnostic. It is a
development instrument, not a replacement for the locked H1/H2 campaigns or a
holdout result.

## Contract

The control arm is exactly `deepseek/deepseek-v4-flash` over the official
Anthropic Messages route `https://api.deepseek.com/anthropic`. The candidate is
exactly `custom_openai/k3` over Kimi's OpenAI Chat Completions route
`https://api.kimi.com/coding/v1`.

Each arm runs the same task list and repetition order in fresh Forge workspaces.
The controller requires one matching `eval_execution_identity` event per trace,
rejects fallback events, requires known provider cost, and refuses to publish a
winner when either arm is not trusted. Pairing uses `(repetition, task_id)`;
task-order, dataset-fingerprint, route, and attempt-count drift fail closed.
The report includes deterministic paired bootstrap intervals for success,
verification, scope, rounds, duration, and cost, plus a canonical JSON artifact
and `.sha256` sidecar. Artifact publication is fail-closed: a comparison must
carry a non-zero source commit and dirty-state digest, and the canonical bytes
are scanned for token-like material before either the JSON or its digest sidecar
is written.

## Run

```bash
cd apps/eval-runner
uv run python -m app.cli provider-ab \
  --cases eval_cases/forge-session-capitalize/case.json \
  --output /tmp/forge-provider-ab.json \
  --trials 1 \
  --require-known-cost \
  --max-total-cost-usd 0.20
```

The command intentionally does not use the operational Kimi→DeepSeek fallback:
a paired comparison must not mix providers. For ordinary development runs, use
`--provider-chain kimi-k3-deepseek`; that chain remains the Kimi-primary,
quota/auth fallback path. Credentials are read from the process environment and
never serialized into the artifact.

`winner` is `candidate` or `control` only when the quality-gated interval
excludes zero without a verification or scope regression. Otherwise it is
`no_material_difference`; incomplete or untrusted evidence is `blocked`.

The offline contract is:

```bash
scripts/acceptance.sh --only "development Kimi/DeepSeek paired diagnostic contract"

# Re-verify a saved artifact without contacting either provider.
cd apps/eval-runner
uv run python -m app.cli provider-ab-verify --artifact /tmp/forge-provider-ab.json
```

## Live smoke evidence

On 2026-08-10, one `forge-session-capitalize` task was run once per arm from
the current worktree. Both arms were trusted, verified, and scope-clean, with
known usage and no fallback events. DeepSeek completed in 10 model rounds at
`$0.002169`; Kimi completed in 7 rounds at `$0.0355908`. The paired decision
was `no_material_difference` because one pair cannot establish a quality
difference. The canonical local artifact digest was
`51db077aeba620ada4ef305669cbc3a9998a49fef03e79712aaeaed4403639cf`.

A complete 10-case development split was then run once per arm from the same
source and dataset fingerprint. Both arms were 10/10 trusted, 10/10 verified,
scope-clean, known-cost, and fallback-free. DeepSeek averaged 16.7 model
rounds, 64,454.9 ms per task, and `$0.028906`; Kimi averaged 13.5 rounds,
62,729.3 ms, and `$0.6642198`. The paired delta was `-3.2` model rounds for
Kimi, while the success and verification deltas were both `0.0`; bootstrap
selected `no_material_difference` (round interval `[-6.0, -0.3]`, duration
interval `[-24,987.5, 15,664.4]` ms). The raw local artifact digest was
`e251d528d158443cb5d2f45ff6f85236969fcb41bd344d1a101f742937a64608`.
This is the strongest current development evidence for an efficiency
difference, not a claim of superior model intelligence or frontier quality.
