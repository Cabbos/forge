# ForgeBench DeepSeek H1 Paid Baseline

H1 is a pinned, attributable baseline for the current Forge harness. It is not
a frontier claim. The campaign is **development only**: it runs the 10 frozen
ForgeBench development cases three times and performs **no holdout** access or
execution.

## Frozen identity and price

- Provider: `deepseek`
- Model: `deepseek-v4-flash`
- Transport: `anthropic_messages`
- Profile: `deepseek-v4-flash-anthropic-baseline-v1`
- Official price source:
  <https://api-docs.deepseek.com/zh-cn/quick_start/pricing/>
- Price snapshot retrieved 2026-08-07: cache-hit input **0.02 CNY** per
  million tokens, cache-miss input 1 CNY per million tokens, and output 2 CNY
  per million tokens.

The profile, dataset lock, source commit, dirty-state digest, provider, model,
transport, and harness fingerprint are frozen into every campaign and attempt.
A mismatched identity or changed source blocks the campaign.

## Spend and recovery boundary

H1 has an 8 CNY campaign allocation, a 1 CNY reservation ceiling per task, and
a 100 CNY global ceiling for the full harness program. Exact known usage is
accounted in CNY from canonical DeepSeek provider events. For the Anthropic
transport, uncached `input_tokens`, cache-creation tokens, and cache-read tokens
are additive; the cache-miss price applies to uncached plus cache-creation
tokens, while the cache-hit price applies only to cache reads. Other transports
retain their inclusive-input semantics. Missing, malformed, legacy-only,
mismatched, or over-cap usage fails closed.

Before any pending marker, attempt artifact, or budget record is created,
`baseline run` executes a typed credential preflight against the exact pinned
Forge headless command, provider, and model. The headless preflight resolves
credentials only: it does not create an agent session, does not start a
provider call, and never serializes the credential. H1 explicitly opts into the
canonical provider's system credential reference so that an isolated eval HOME
does not hide the desktop-managed key, but credential resolution checks the
controller environment first. The local paid campaign uses
`DEEPSEEK_API_KEY` from `apps/eval-runner/.env`; that file is Git-ignored,
permissioned `600`, and must be explicitly exported with
`set -a; source .env; set +a` before invoking the CLI. The child HOME, profiles,
logs, registry, Continuity state, and shell environment remain isolated, and
Python bytecode plus pytest cache output are redirected into that isolated
runtime state rather than the task workspace. This prevents a verification-only
no-op task from acquiring a synthetic cache-file patch. The credential value is
never serialized. A missing credential, identity
mismatch, malformed response, command failure, or 30-second timeout returns
`error: baseline_run_preflight_failed` and leaves campaign metadata, `pending/`,
`attempts/`, and `budget.jsonl` unchanged.

Before a provider call, the controller atomically writes a
`provider_call_pending` record. A completed attempt publishes its immutable
artifact, appends a hash-chained budget entry, and only then removes the pending
record. Recovery never reissues a pending paid call. If interruption makes the
actual charge unknowable, recovery reserves the full 1 CNY and records
`interrupted_paid_attempt_unknown_cost`; the campaign stops for review.

## Explicit CLI

Every command requires an explicit campaign and profile. Mutation commands
never select a “latest” campaign.

```bash
cd apps/eval-runner

uv run python -m app.cli baseline init \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --campaign-id h1-anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json

uv run python -m app.cli baseline run \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json \
  --max-attempts 1

uv run python -m app.cli baseline status \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json

uv run python -m app.cli baseline recover \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json

uv run python -m app.cli baseline report \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json

uv run python -m app.cli baseline verify \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

Run requires `FORGE_EVAL_FORGE_AGENT_COMMAND` to point at the Forge headless
adapter. The same command handles both preflight and the paid attempt. The
command and its environment are not serialized into campaign metadata or
compact CLI output. `status` and `verify` are read-only; `report` may create or
replace only the compact report and artifact index, never the budget ledger or
attempt artifacts.

The offline contract is:

```bash
scripts/acceptance.sh --only "DeepSeek H1 paid-baseline contract"
```

## Evidence status

The H1 controller and offline acceptance contract are implemented. Paid
diagnostic campaign `h1-anthropic-control-v3` reached five terminal attempts
from source `f0dc32d3910a61480a8f91735496c7055e9d2c00` for
`0.14549788 CNY`: four attempts were trusted and one verification-only no-op
attempt failed closed because pytest cache files appeared as an unreplayable
patch. The controller stopped without retrying. That immutable evidence led to
the out-of-workspace cache isolation above; it is not presented as a completed
baseline.

No baseline result is claimed until a subsequent clean-source campaign has
completed, `baseline verify` returns `ok: true`, and the compact report plus
SHA-256 artifact index are published. H1 does not open, score, or make any
statement about the sealed holdout split.
