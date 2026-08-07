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
| H2 | official Anthropic vs OpenAI Chat Completions route attestation and conformance | transport/harness A/B wiring; no Responses claim, winner, or holdout |
| H3 | minimal context manifest, bounded tool envelopes, typed stagnation policy | offline convergence evidence; no paid score improvement |
| H4 | locked 60-attempt holdout report/decision gate plus provenance campaign contract | release decision boundary and auditable monorepo packaging; an actual holdout result still requires paid evidence and finalized campaign history |

## Reproducible gates

From the repository root:

```bash
scripts/acceptance.sh --only "ForgeBench canonical monorepo and frozen dataset"
scripts/acceptance.sh --only "Forge-built monorepo provenance contract"
scripts/acceptance.sh --only "DeepSeek H1 paid-baseline contract"
scripts/acceptance.sh --only "DeepSeek H2 official transport A/B contract"
scripts/acceptance.sh --only "frontier harness contract evidence"
scripts/acceptance.sh --only "ForgeBench frozen holdout release gate"
scripts/acceptance.sh --list-json
```

The matrix currently contains 85 executable gates. `--list-json` is the
machine-readable source for release tooling; `--results-json` binds executed
results to the current commit. The H3 and H4 contract gates are intentionally
offline and safe to run without a provider key; they prove decision boundaries,
not paid model-quality results.

## Current boundary

The paid diagnostic is recorded in
[`forgebench-h2-paid-diagnostic.md`](forgebench-h2-paid-diagnostic.md). Both
official DeepSeek routes pass smoke calls, but the development campaigns stop
on a reproducible bulk-update task with unknown charge and are recovered
without replay. Therefore there is no complete trusted 30-attempt arm, no
paired A/B winner, and no valid 20-case holdout claim yet.

This document proves that the controls and evidence paths live together in the
Forge monorepo and are executable. It does not claim that the controller-owned
baseline was written by Forge, nor does a passing contract gate alone prove
model authorship or frontier-level quality. Those claims require a finalized
`FORGE-BUILD-MANIFEST.json` whose ledger and patch chain pass
`campaign verify`.
