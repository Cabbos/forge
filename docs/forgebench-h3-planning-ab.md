# ForgeBench H3 planning-policy evidence

This document records the first paid development measurements of the isolated
`planning_v1` profiles. These are development-split experiments only; they do
not open or score the sealed holdout.

## Route A/B with planning held constant

Source `880b29de141c066a5fdac2123d589b4757a6c7b9`, dataset
`39ac183a1f3f8e7c0f29dbbe5d4e302765935d94417f8c2adffb984632223dbd`, and the
same dirty-state digest were used for both complete 30-attempt arms:

| Arm | Route | Trusted success | Scope violations | Median rounds | Cost (CNY) |
| --- | --- | ---: | ---: | ---: | ---: |
| control | Anthropic Messages + `planning_v1` | 28/30 (0.9333) | 2/30 (0.0667) | 22.0 | 0.70695380 |
| candidate | OpenAI Chat Completions + `planning_v1` | 26/30 (0.8667) | 3/30 (0.1000) | 29.5 | 0.72080008 |

The canonical comparison digest is
`3e72d85d9ac6021f05cb9d612b980ce4a65d3f688f88a1924070594fb2c1a198`.
The selection is `no_material_difference`; the candidate point estimate is
lower success and 7.5 more median model rounds, so it is not promoted as the
preferred route.

## Same-route policy A/B

`policy_ab_v1` now permits only a same-route pair whose sole non-route profile
difference is `minimal_v1` versus `planning_v1`. On source
`dff078392ab38b50a1843736827f9ac639096d79` and the same dataset, both arms
used Anthropic Messages:

| Arm | Policy | Trusted success | Scope violations | Median rounds | Cost (CNY) |
| --- | --- | ---: | ---: | ---: | ---: |
| control | `minimal_v1` | 25/30 (0.8333) | 5/30 (0.1667) | 25.5 | 0.61027644 |
| candidate | `planning_v1` | 26/30 (0.8667) | 3/30 (0.1000) | 25.0 | 0.64757024 |

The paired bootstrap point estimates are +0.0333 success, -0.0667 scope
violation rate, and -0.5 median rounds, but all quality intervals cross zero;
the canonical comparison therefore remains `no_material_difference`. The
planning prompt is an evidence-backed hypothesis, not a default or frontier
claim. Its fingerprint is retained so a later larger sample can be compared
without policy drift.

The policy comparison digest is
`1a467583e2971a88b52fe64623464a853e48f8beaf41e4f664b59bd6a842d7ab`.

## Harness boundary learned during the run

The first planning control run stopped safely at 22 calls because the model
returned disposable `.uv-cache` files in its patch list; workspace snapshots
already allowlisted those caches, but patch replay had not. The fix in
`880b29de` filters only the explicit `.bun-cache`/`.uv-cache` directories before
replay, and the replacement control arm completed 30/30 with no trust blocker.
This is retained as a harness regression lesson rather than silently deleting
the blocked evidence.
