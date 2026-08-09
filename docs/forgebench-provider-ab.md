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
The comparison also records its run contract (trials, cost/runtime ceilings,
known-cost requirement, and red-team selection) so the paid experiment can be
reconstructed from the artifact rather than inferred from the shell command.
The report includes deterministic paired bootstrap intervals for success,
verification, scope, rounds, duration, cost, and every universal score dimension;
contract-bound arm summaries also include deterministic median and p95 rounds,
duration, and cost so long-tail convergence failures remain visible;
they also carry per-arm failure categories and candidate-minus-control category
deltas for choosing the next harness intervention;
the candidate cannot win when any safety score regresses. It also writes a
canonical JSON artifact and `.sha256` sidecar. Artifact publication is fail-closed: a comparison must
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
  --max-total-cost-usd 0.20 \
  --max-total-runtime-seconds 1200
```

The command intentionally does not use the operational Kimi→DeepSeek fallback:
a paired comparison must not mix providers. For ordinary development runs, use
`--provider-chain kimi-k3-deepseek`; that chain remains the Kimi-primary,
quota/auth fallback path. Credentials are read from the process environment and
never serialized into the artifact.

`winner` is `candidate` or `control` only when the quality-gated interval
excludes zero without a verification or scope regression. Otherwise it is
`no_material_difference`; incomplete or untrusted evidence is `blocked`.
Contract-bound runs also require at least two paired attempts before making a
capability claim; a one-pair smoke is recorded but remains `blocked`.
If the wall-clock budget expires, the run is cancelled and no incomplete pair
is published.

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

After the runtime-budget, artifact-verifier, and universal-score changes, a
fresh route-pure smoke on `forgebench-dev-domain-normalize-title` completed
1/1 trusted, verified, and scope-clean on both arms. DeepSeek used 17 rounds,
98,860 ms, and `$0.00489`; Kimi used 15 rounds, 69,779 ms, and `$0.0624`.
All universal score deltas were `0.0`, so the one-pair decision remained
`no_material_difference`. The current-source artifact digest is
`4de283b60d15e3f6b816c56f18ba9e458c0c7cfc4a2bd7908a25f23d7dd92dd2`, bound to
commit `0c953e292c58305f8d4a92e1524211863513651b`; the offline verifier passed.
This is a current-harness smoke, not a model-quality or frontier claim.

The follow-up smoke after run-contract provenance was added completed the same
case with the contract bound in the artifact (`trials=1`, known-cost required,
`$0.20` per-arm ceiling, 600-second wall-clock ceiling). Both arms remained
trusted, verified, and scope-clean; Kimi used 18 rounds and DeepSeek 21. The
universal score delta was `+1.0` for `future_state_leakage_ok` (Kimi 1.0,
DeepSeek 0.0), while functional correctness and all other reported universal
scores matched. This is a safety signal to investigate with more pairs, not a
model-quality win. The verifier passed artifact
`3eb92154dd6cf453cfcfc74f7727243f25e60c039af8cba43fff4d9b10b42c01`, bound to
commit `3645ad1492f4307ff320cc93fea9b8be44eb9fed`.
