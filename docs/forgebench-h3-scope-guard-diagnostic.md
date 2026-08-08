# ForgeBench H3 scope-guard diagnostic

This document records a development-only paid diagnostic after the workspace
scope and environment-isolation hardening. It is not a holdout result and does
not establish a frontier-quality claim.

The campaign artifacts below were produced before commit
`76d7822c6e4faf1ed402cc16c5b6c4e265c34d61`, which corrected the Rust event
summary to count model-request transitions instead of counting individual
parallel tool starts. The stored round metrics remain immutable historical
evidence, but they are not authoritative for the current round threshold;
future paid campaigns must be run from the corrected source.

## Locked inputs

- Source commit for both arms: `05bfc6c83eda52d6f6f699497dd84a158856c966`;
- Dataset fingerprint: `39ac183a1f3f8e7c0f29dbbe5d4e302765935d94417f8c2adffb984632223dbd`;
- Raw campaign root: `/private/tmp/forge-h3-scope-guard-v1.CaAKKz` (outside Git);
- Arms: Anthropic `minimal_v1` control and Anthropic `planning_v1` candidate;
- Each arm was pinned to the 10-case development split × 3 repetitions.

## Results

The control arm stopped fail-closed at call 23 after 22 terminal attempts,
with `interrupted_paid_attempt_unknown_cost` and known spend `0.54420104 CNY`.
It was not replayed and no comparison was emitted.

The planning candidate closed all 30 attempts. Its canonical frozen report is
`baseline-report.json` SHA-256
`96f6fadeb1382716bb0703765fb2e40ef3d6dfb25d8673c4ab4d63c80c65b014`; its
artifact index SHA-256 is
`4c08c6732bcc33076ec1754003e2e79bbf42f5fc5d7ade0efc023948a3a9f19c`.
`baseline report --frozen` and `baseline verify --frozen` both succeeded.

| Metric | Candidate |
| --- | ---: |
| Trusted success | 27/30 (0.9000) |
| Scope-failure attempts | 2/30 (0.1000) |
| Verification pass | 30/30 (1.0000) |
| Median model rounds | 24.5 |
| Repeated tool-call rate | 0.7883 |
| Known cost | 0.80588248 CNY |

The environment hardening removed the earlier `.venv` observation noise from
the successful attempts, but one model arm still created `uv.lock` and one
attempt exceeded the model-round budget. The evidence therefore supports the
scope/route boundary and frozen publication contract, not threshold promotion:
the frontier gate still requires scope ≤2% and median rounds ≤12.

Next iteration should isolate uv configuration before the first model call and
target repeated-tool convergence on the development split. The sealed holdout
must remain untouched until a complete, trusted development comparison passes.
