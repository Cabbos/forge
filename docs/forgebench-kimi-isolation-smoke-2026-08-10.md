# ForgeBench Kimi K3 post-isolation smoke — 2026-08-10

This development-only smoke validates the controller-side environment boundary
after commit `d10b0425` (`fix(eval): isolate controller validation commands`).
It is not a holdout or frontier-quality result.

## Locked run shape

- Source commit: `d10b0425`
- Case: `forgebench-dev-config-env-parser`
- Provider chain: Kimi `k3` primary, DeepSeek fallback enabled but unused
- Cost gate: `--require-known-cost --max-total-cost-usd 1.00`
- Raw artifact: outside Git at `/tmp/forge-kimi-isolation-smoke.json`
- Credentials: inherited through environment variables only; no values are
  recorded here or in the repository

## Evidence

| Metric | Result |
| --- | ---: |
| Success rate | 1.0000 |
| Verification pass rate | 1.0000 |
| Scope violation rate | 0.0000 |
| Model rounds | 11 |
| Kimi K3 cost | `$0.0521442` |
| Provider usage known rate | 1.0000 |
| Provider cost known rate | 1.0000 |
| Fallback events | 0 |

The trace contains both `eval_runtime_isolation` and `eval_cache_isolation`
events, and the controller-side setup/validation commands completed under the
same isolated environment as the headless agent. This closes the previously
observed path where controller checks could create `.venv`, `uv.lock`, or cache
files in the observed workspace.

The result is a targeted regression smoke, not evidence that the H4 release
thresholds have passed.
