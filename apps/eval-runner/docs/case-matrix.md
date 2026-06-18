# Eval Case Matrix

The eval suite is split into independently runnable lanes. Red-team cases stay out of normal success-rate runs unless `--include-red-team` or `--red-team-only` is passed.

| Lane | Tags | Default Command |
| --- | --- | --- |
| Core edit | `core-edit`, `small-edit`, `tool-use` | `uv run python -m app.cli --cases eval_cases --provider mock` |
| Continuity pipeline | `continuity-pipeline`, `sqlite-assertions` | `uv run python -m app.cli --cases eval_cases/continuity-pipeline-task-summary --provider forge --model local-forge` |
| Desktop runtime | `desktop-runtime` | `uv run python -m app.cli --cases eval_cases/desktop-permission-rules-precedence --provider mock` |
| Failure recovery | `failure-recovery`, `validation`, `timeout` | `uv run python -m app.cli --cases eval_cases --provider mock --min-success-rate 0.1` |
| Agent loop | `agent-loop`, `stop-reason` | `uv run python -m app.cli --cases eval_cases/agent-loop-tool-loop-detected --provider mock` |
| Red team | `red_team` | `uv run python -m app.cli --cases eval_cases --provider mock --red-team-only --max-red-team-failure-rate 0` |
| Promoted trace | `promoted-trace` | `uv run python -m app.cli --cases eval_cases/promoted --provider mock` |

Every executable case should include validation or verification commands, expected changed files, forbidden changed files, and a fixture path when the case expects real workspace edits.

## Current Coverage

The suite currently contains 43 loadable `case.json` files across core edit, continuity pipeline, desktop runtime, failure recovery, agent-loop, red-team, and promoted-trace lanes.

## Acceptance Commands

```bash
uv run pytest tests/test_case_expansion_matrix.py tests/test_cases.py tests/test_continuity_eval_cases.py -v
uv run python -m app.cli --cases eval_cases --provider mock --min-success-rate 0.1 --max-scope-violation-rate 0.2
uv run python -m app.cli --cases eval_cases --provider mock --red-team-only --max-red-team-failure-rate 0
uv run python -m app.cli --cases eval_cases/promoted --provider mock --min-success-rate 0.1
uv run python -m app.cli case-lifecycle --cases eval_cases
```
