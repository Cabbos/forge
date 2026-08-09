# Kimi K3 development run with headless write policy — 2026-08-10

This is a development-only diagnostic after the eval-only Rust write-policy
change. It is not a holdout result and does not establish a frontier-quality
claim.

## Identity and budget

- Harness implementation commit: `30726ffb`
- Explicit provider chain: `kimi-k3-deepseek`
- Primary model: Kimi `k3` through the approved OpenAI-compatible route
- Fallback: isolated DeepSeek V4 Flash only for authentication/quota/rate-limit
  errors
- Cases: all 10 `forgebench/cases/development` tasks, one attempt each
- Gate: `--require-known-cost --max-total-cost-usd 1.20`
- Credentials: read from CCSwitch into process environment variables only; no
  secret value was written to the artifact or repository

## Aggregate result

| Metric | Result |
| --- | ---: |
| tasks | 10 / 10 |
| success | 1.0000 |
| verification | 1.0000 |
| scope violations | 0 / 10 |
| average model rounds | 11.6 |
| average confirmation requests | 5.5 |
| input tokens | 369,392 |
| output tokens | 20,585 |
| provider usage known | 1.0000 |
| provider cost known | 1.0000 |
| total cost | `$0.5847462` |
| fallback events | 0 |

Every task changed only an expected file (the no-op task changed nothing), and
every controller validation passed. The controller sent each task's expected
and forbidden paths as `write_policy`; no task needed an
`eval_write_policy_block`, so the run demonstrates no quality regression and
the policy's normal in-scope path, not a measured reduction in model-originated
scope mistakes.

Compared with the earlier 10-case Kimi diagnostic (`11.4` average rounds,
`$0.6010896`, zero scope violations), this sample is directionally similar and
does not support a claim of material intelligence improvement. The concrete
gain is a pre-execution safety boundary plus auditable evidence when it fires.

Artifact: `/tmp/forge-kimi-write-policy-development.json` (raw local
diagnostic; outside Git).
