# ForgeBench H4 Holdout Release Gate

H4 is the release boundary for the frozen ForgeBench holdout, not a tuning
loop. The report contract requires 20 sealed cases × 3 repetitions (60
attempts). Reports should be derived with `summarize_holdout_attempts` from one
immutable evidence record per case/repetition pair; hand-authored aggregate
numbers cannot replace those records. The derived report records the selected
model, official transport, dataset fingerprint, harness fingerprint, budget,
source commit, profile identity, price-table fingerprint, and every
evidence-coverage rate.

The locked thresholds are:

- trusted success rate ≥ 80%;
- changed-file scope violations ≤ 2%;
- median model rounds ≤ 12;
- score and harness-contract coverage = 100%;
- workspace observation, patch replay, verification evidence, known usage, and
  terminal failure taxonomy = 100%;
- total cost ≤ 35 CNY.

Verify a canonical report with:

```bash
cd apps/eval-runner
uv run python -m app.cli holdout verify --report /path/to/holdout-report.json
```

The paid campaign boundary is explicit and resumable. Initialize it only from
the pinned development comparison and the checked-in sealed split:

```bash
uv run python -m app.cli holdout init \
  --campaign /private/tmp/forge-h4/arm \
  --campaign-id deepseek-h4-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json \
  --development-comparison /private/tmp/forge-h2-campaigns-v6/comparison-v1.json
uv run python -m app.cli holdout run --campaign /private/tmp/forge-h4/arm \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
uv run python -m app.cli holdout recover --campaign /private/tmp/forge-h4/arm \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
uv run python -m app.cli holdout report --campaign /private/tmp/forge-h4/arm \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

`holdout run` writes `pending/<attempt>.json` before invoking Forge and appends
an fsynced hash-chained budget entry before deleting it. A crash or unknown
usage closes that attempt as terminal unknown-cost evidence; it is never
replayed. A complete campaign publishes `holdout-artifact-index.json`, the
canonical report, and its SHA-256 sidecar. Raw campaign directories remain
outside the repository; only code, tests, dataset locks, and provenance
documents are checked in.

The verifier rejects non-canonical JSON, incomplete attempts, trust gaps,
missing evidence, budget overruns, and threshold failures. A trusted
integration trace must also carry controller-owned `eval_agent_identity`
evidence matching the campaign's provider, exact model, transport, and harness
fingerprint, plus a passing `harness_contract_evidence` taxonomy event. Artifact
wrapper fields alone are not sufficient proof of Forge authorship. A failing or
missing report does not authorize another holdout attempt; improvements must
return to the development split with a new harness fingerprint.

The offline contract gate proves the decision boundary only. Until a real
60-attempt report passes from a clean checkout and the provenance campaign is
verified, Forge does not claim a holdout result or frontier-grade KPI.
