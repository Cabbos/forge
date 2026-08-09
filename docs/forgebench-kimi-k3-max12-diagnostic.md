# ForgeBench Kimi K3 12-Round Diagnostic

This document records a development-only probe of the Forge harness after
`fc7c4bc3`. It is diagnostic evidence, not a holdout or release result.

## Locked run

- Source commit: `fc7c4bc3`
- Provider chain: Kimi K3 primary with DeepSeek fallback
- Model: `k3`
- Development cases: 10
- Per-task model-round cap: 12
- Total known cost: `$0.48975180`
- Provider usage and cost known rate: `1.0000`

The cases were copied to a temporary directory and their task-level round cap
was set to 12. The temporary copy was used only to measure convergence and was
not treated as a canonical dataset. Its report trust check correctly rejected
the missing fixture paths, so this run cannot be used as release evidence.

## Observed result

| Metric | Observed |
| --- | ---: |
| Completed tasks | 0/10 |
| Verification pass rate | 0.0000 |
| Scope violation rate | 0.0000 |
| Average model rounds | 11.1 |
| Budget-exhausted tasks | 7/10 |
| Verification-failed tasks | 3/10 |

The cap was enforced: no task exceeded 12 model rounds. The limiting failures
were `max_model_rounds_exceeded` for seven tasks and `verification_failed` for
three tasks where the runner found no safe verification command. This is useful
convergence evidence, but it is not evidence of task quality: functional,
regression, and bug-fix scores were all `0.0` because no task completed the
verification gate.

## Follow-up implication

The next harness iteration should improve safe verification discovery and
completion behavior before another paid holdout run. The 12-round cap should
remain an explicit experiment/profile setting rather than changing historical
H1/H2 profile bytes or weakening the holdout thresholds.
