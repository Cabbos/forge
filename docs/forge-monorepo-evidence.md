# Forge Monorepo Evidence Index

This is the audit entry point for the Forge-built evaluation monorepo. It
keeps product code, eval controls, acceptance commands, and provenance rules
in one repository while preserving the boundary between offline contract
evidence and paid model-quality evidence.

## Repository scope

| Surface | Role | Authoritative checks |
| --- | --- | --- |
| `apps/desktop` | Forge runtime, adapters, headless route boundary | Rust library tests, desktop backend checks, acceptance matrix |
| `apps/eval-runner` | ForgeBench catalog, controller, runner, scoring, trust, reports | `uv run pytest`, Ruff, mypy, H1/H2/H3/H4 gates |
| `apps/website` | Product website prototype | `npm run build:website` |
| `scripts/acceptance.sh` | Single repo-wide gate matrix | `--dry-run`, `--list-json`, exact gate execution |

The ForgeBench fixture itself is a versioned TypeScript/Python monorepo. Its
controller-authored baseline is locked and is not claimed as Forge-generated.
Only the integration campaign's explicitly accepted, trusted Forge patches can
enter a post-baseline product history; see
[`forgebench-provenance.md`](forgebench-provenance.md).

## Evidence layers

| Layer | Current evidence | Claim boundary |
| --- | --- | --- |
| H0 | isolated runtime, workspace observer, patch replay, frozen 10/20 dataset | harness trust and dataset integrity; no frontier KPI |
| H1 | DeepSeek Anthropic control, budget ledger, unknown-cost recovery | development paid baseline machinery; no complete 30-case arm |
| H2 | official Anthropic vs OpenAI Chat Completions route attestation and conformance | complete v6 development comparison (`no_material_difference`); no Responses or holdout claim |
| H3 | minimal context manifest, bounded tool envelopes, typed stagnation policy | offline convergence evidence; no paid score improvement |
| H4 | sealed 60-attempt holdout campaign runner, locked report/decision gate, and provenance campaign contract | resumable release boundary and auditable monorepo packaging; an actual holdout result still requires paid evidence and finalized campaign history |

## Reproducible gates

From the repository root:

```bash
scripts/acceptance.sh --only "ForgeBench canonical monorepo and frozen dataset"
scripts/acceptance.sh --only "Forge-built monorepo provenance contract"
scripts/acceptance.sh --only "DeepSeek H1 paid-baseline contract"
scripts/acceptance.sh --only "DeepSeek H2 official transport A/B contract"
scripts/acceptance.sh --only "frontier harness contract evidence"
scripts/acceptance.sh --only "ForgeBench frozen holdout release gate"
scripts/acceptance.sh --only "ForgeBench sealed holdout campaign runner"
scripts/acceptance.sh --list-json
```

The matrix currently contains 86 executable gates. `--list-json` is the
machine-readable source for release tooling; `--results-json` binds executed
results to the current commit. The H3 and H4 contract gates are intentionally
offline and safe to run without a provider key; they prove decision boundaries,
not paid model-quality results.

## Current boundary

The paid diagnostic is recorded in
[`forgebench-h2-paid-diagnostic.md`](forgebench-h2-paid-diagnostic.md). Both
official DeepSeek routes pass trusted smoke calls, and the v6 source-pinned
development campaigns now complete 30 attempts per arm with independent
verification. The paired decision is `no_material_difference`; there is still
no valid 20-case holdout claim.

The latest offline hardening also isolates Bun/uv cache roots under each
attempt's runtime state. Only the explicitly named `.bun-cache` disposable
directory is filtered by both workspace observers; source files remain in the
observed and replayed patch boundary. This closes the cache false-positive
path without weakening fail-closed report/observation mismatch checks.

The first post-fix paid pair (`bda5d977`) reached six trusted attempts per arm
before both arms stopped at the same unknown-cost multi-file API interruption;
recovery reserved the bounded ceiling and performed no replay. The resulting
`h2-control-v4`/`h2-candidate-v4` ledgers are diagnostic only: there is still no
complete 30-attempt arm, paired winner, or holdout claim.

Offline root-cause analysis then found two secret-scrub false positives in the
v4 artifact publication path: source-level
`api_key = os.getenv('DEEPSEEK_API_KEY')` and the embedded `sk-api` substring in
the `task-api-bulk-update` temporary workspace path. The scrubber now requires
token-shaped `sk-...` or Bearer values at an identifier boundary, with
regression coverage for both paths and real token rejection. The historical v4
artifacts remain immutable and are not replayed.

A fresh pair pinned to `f72a0b4c` first completed a bounded 7-attempt arm on
each official route. Both arms verified with seven trusted, known-usage,
verification-passing artifacts and zero scope violations (`0.14829088 CNY`
control, `0.15879656 CNY` candidate); the control `task-api-bulk-update`
artifact now publishes normally. This closed the observed artifact-publication
boundary before the pair was extended to the full development split.

That same pair subsequently completed 30 attempts per arm and passed independent
campaign verification. Control was 30/30 trusted with 0.9000 trusted success,
0.1000 scope violations, median 25.0 model rounds, and `0.74215116 CNY`; the
candidate was 30/30 trusted with 0.9333 trusted success, 0.0333 scope
violations, median 27.5 rounds, and `0.73471284 CNY`. Usage was known and
verification passed for every attempt. The canonical paired comparison
(`a6bd277a04ef2810d97c88096690c3a10567d6ec73d9fc625575bfda0c2b9f2b`) selected
`no_material_difference`. This is a frozen development result, not a frontier
or holdout claim; the H4 release gate remains separate.

This document proves that the controls and evidence paths live together in the
Forge monorepo and are executable. It does not claim that the controller-owned
baseline was written by Forge, nor does a passing contract gate alone prove
model authorship or frontier-level quality. Those claims require a finalized
`FORGE-BUILD-MANIFEST.json` whose ledger and patch chain pass
`campaign verify`.
