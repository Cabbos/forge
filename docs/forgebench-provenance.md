# Forge-built Monorepo Provenance

ForgeBench has two deliberately separate tracks:

- The **scoring track** runs independent attempts and preserves their traces,
  reports, patches, trust decisions, cost, and benchmark scores as immutable
  evidence.
- The **integration track** builds one product history. Only a trusted,
  passing Forge patch selected by exact task, trace, report, and patch paths may
  enter that history.

Passing a scoring attempt does not integrate it, and integrating a patch does
not count as a holdout success. This separation prevents cherry-picking or
controller edits from being presented as model performance.

## Provenance boundary

The canonical ForgeBench fixture is a controller-authored benchmark scaffold.
Its locked baseline commit is the root of the campaign, but it is not claimed
as Forge-authored work. After campaign initialization, the accepted product
commit chain may contain only patches emitted by Forge runs that pass all of
these checks:

1. provider, model, transport, dataset, harness, input commit, and workspace
   observation match the frozen campaign inputs;
2. the trace and report are trusted and their exact bytes match the supplied
   SHA-256 digests;
3. the supplied patch exactly matches the independently observed workspace
   patch, stays inside task scope, and replays from the declared input commit;
4. visible, hidden, and full verification pass in isolated copies;
5. the resulting commit has the prescribed message, trailers, parent, and Git
   tree.

Artifacts and the append-only hash-chain ledger live outside the agent-visible
product worktree. Duplicate task IDs, run IDs, patches, or output commits are
rejected. Secret-like content is rejected rather than copied into provenance
artifacts.

The only allowed controller interventions are campaign initialization and the
final manifest-only commit. Any human or controller product-source edit after
initialization invalidates the current campaign. Recovery may finish or roll
back an already prepared transaction, but it may not synthesize a new product
change.

## Operator workflow

Run these commands from `apps/eval-runner`. Every mutation names a campaign;
`accept` additionally requires exact task, trace, report, and patch inputs. The
CLI never selects a “latest” artifact.

```bash
uv run python -m app.cli campaign init \
  --campaign output/campaign-h1 \
  --harness-profile forgebench/harness-profile.lock.json

uv run python -m app.cli campaign accept \
  --campaign output/campaign-h1 \
  --task-id dev-01 \
  --trace output/runs/dev-01.trace.json \
  --report output/runs/dev-01.report.json \
  --patch output/runs/dev-01.patch

uv run python -m app.cli campaign status \
  --campaign output/campaign-h1

uv run python -m app.cli campaign recover \
  --campaign output/campaign-h1

uv run python -m app.cli campaign finalize \
  --campaign output/campaign-h1

uv run python -m app.cli campaign verify \
  --campaign output/campaign-h1
```

`status` and `campaign verify` are read-only. `recover` is idempotent and
handles interrupted acceptance/finalization boundaries without accepting a
different patch.

## What the manifest proves

Finalization creates a controller commit containing only
`FORGE-BUILD-MANIFEST.json`. Its `final_commit` identifies the last Forge
product commit; its parent/commit scope makes the manifest wrapper independently
recognizable. The manifest binds:

- the locked fixture, development/holdout dataset, and harness profile;
- every accepted task and the ordered external provenance ledger;
- the raw trace, report, patch, and verification artifact index;
- the final Forge product Git tree;
- an independent full-verification command, exit status, commit, and output
  digests;
- all declared controller interventions.

`campaign verify` recomputes those hashes, verifies every ledger link, checks
the exact Git history, replays each patch from its input commit in a temporary
clone, compares the replayed tree with the recorded output commit, and runs the
full verifier on a clean copy. A source, history, ledger, artifact, lock,
manifest, or verification-field change fails closed.

The scoring and integration tracks can be combined only through the offline
frontier gate:

```bash
uv run python -m app.cli release verify \
  --holdout-campaign /path/to/sealed-h4-campaign \
  --campaign /path/to/forge-built-campaign
```

It re-verifies the sealed H4 attempts, budget chain, trust decisions, artifact
index, and canonical report before checking the H4 threshold decision,
profile/dataset identity binding, and the complete Forge campaign as one
machine-readable decision. A hand-authored aggregate report cannot satisfy the
gate, and it never starts a provider call.

This proves that every product change after the locked baseline and before the
manifest came from the recorded trusted Forge patch chain, and that the bound
tree passed the recorded independent verification.

It does **not** prove that Forge generated the controller-authored baseline,
that unintegrated attempts were successful, that a model is generally
frontier-level, or that the 20-case holdout passed. H0C made no paid DeepSeek
call. Paid DeepSeek campaign cost and holdout success must be established later
by the frozen scoring protocol and its aggregate KPI report.

## Acceptance gate

The repository acceptance matrix locks this contract under:

```text
Forge-built monorepo provenance contract
```

Its executable command is:

```bash
cd apps/eval-runner &&
uv run pytest tests/test_provenance_models.py tests/test_provenance.py \
  tests/test_campaign.py tests/test_campaign_verify.py
```
