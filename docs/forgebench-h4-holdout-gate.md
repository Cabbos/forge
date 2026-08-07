# ForgeBench H4 Holdout Release Gate

H4 is the release boundary for the frozen ForgeBench holdout, not a tuning
loop. The report contract requires 20 sealed cases × 3 repetitions (60
attempts). Reports should be derived with `summarize_holdout_attempts` from one
immutable evidence record per case/repetition pair; hand-authored aggregate
numbers cannot replace those records. The derived report records the selected
model, official transport, dataset fingerprint, harness fingerprint, budget,
and every evidence-coverage rate.

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

The verifier rejects non-canonical JSON, incomplete attempts, trust gaps,
missing evidence, budget overruns, and threshold failures. A failing or missing
report does not authorize another holdout attempt; improvements must return to
the development split with a new harness fingerprint.

The offline contract gate proves the decision boundary only. Until a real
60-attempt report passes from a clean checkout and the provenance campaign is
verified, Forge does not claim a holdout result or frontier-grade KPI.
