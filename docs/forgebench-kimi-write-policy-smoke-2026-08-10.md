# Kimi K3 headless write-policy smoke — 2026-08-10

This is a development-only runtime smoke for the eval-only write boundary. It
does not change the sealed H1/H2/H4 evidence and is not a frontier or holdout
claim.

## Contract

- Source commit: `30726ffb`
- Provider chain: explicit `kimi-k3-deepseek`
- Primary route: Kimi K3 OpenAI-compatible Chat Completions
- Fallback: isolated DeepSeek V4 Flash on auth/quota/rate-limit errors only
- Case: `forgebench-dev-config-env-parser`
- Budget: `$0.25` maximum cumulative cost, `--require-known-cost`
- Secret handling: Kimi and DeepSeek credentials were read from CCSwitch into
  process environment variables only; no credential value entered the trace or
  repository.

## Result

The real Forge headless binary completed the case with:

| Metric | Result |
| --- | ---: |
| tasks | 1 / 1 |
| success | 1.0000 |
| verification | 1.0000 |
| scope violations | 0 |
| model rounds | 9 |
| input tokens | 18,049 |
| output tokens | 1,298 |
| provider usage known | 1.0000 |
| provider cost known | 1.0000 |
| cost | `$0.0342186` |
| fallback events | 0 |

The trace changed only the expected
`packages/config/src/env.ts` file. The controller sent the expected/forbidden
file contract as `write_policy`; the Rust eval emitter accepted in-scope file
writes and would deny out-of-scope file writes, deletes, and mutating shell
confirmations before execution. No `eval_write_policy_block` was needed in
this clean run, so the typed block path remains covered by Rust unit and
controller contract tests rather than being inferred from a successful trace.

Artifact: `/tmp/forge-kimi-write-policy-smoke.json` (raw local diagnostic;
outside Git).
