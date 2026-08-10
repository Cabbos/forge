# Forge H0C Provenance and Integration Campaign Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task with test-first commits.

**Goal:** Produce machine-verifiable proof that every accepted product change in the final ForgeBench monorepo came from a trusted Forge run.

**Architecture:** Scoring attempts remain immutable evidence. A separate integration campaign starts from the locked canonical fixture, accepts only a trusted trace whose replayable patch matches its authoritative workspace observation, verifies the patch, creates one Git commit, and appends a hash-chained ledger entry outside the agent-visible repository. Finalization writes `FORGE-BUILD-MANIFEST.json` and an artifact index; verification recomputes commits, patches, traces, reports, fixture/dataset/harness hashes, the final tree, and the full build result.

**Tech Stack:** Python 3.11, Pydantic 2, Git CLI, SHA-256 canonical JSON/tree hashing, pytest.

**Design reference:** `docs/superpowers/specs/2026-08-07-forge-frontier-deepseek-harness-design.md`, Sections 7.2, 7.3, and Phase H0.

**GitNexus preflight:** This phase adds new campaign modules and minimally extends `app.cli.main` (MEDIUM risk, 5 direct callers). Run impact on `main`, `write_backtest_artifact`, and `AgentTrace` before editing. Any HIGH/CRITICAL result requires stopping and narrowing the CLI integration.

---

### Task 1: Define canonical provenance and manifest schemas

**Files:**

- Add: `apps/eval-runner/app/provenance_models.py`
- Add: `apps/eval-runner/tests/test_provenance_models.py`

**Step 1: Write failing schema tests**

Test that a ledger entry:

- requires schema version 1;
- fixes `agent="forge"`;
- requires `trust_status="trusted"`;
- validates every digest as 64 lowercase hex characters;
- requires nonnegative `cost_cny`;
- forbids extra fields and secrets;
- round-trips through canonical JSON.

Test that a manifest:

- references fixture, development/holdout dataset, harness, ledger, final tree, artifact index, and final verification digests;
- contains an ordered accepted task list;
- declares controller interventions;
- cannot claim `complete` with an intervention after campaign start, a broken ledger, or a failed final verification.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_provenance_models.py -q
```

Expected: FAIL because the schemas do not exist.

**Step 3: Implement strict Pydantic models**

Add these core models:

```python
Sha256 = Annotated[str, StringConstraints(pattern=r"^[0-9a-f]{64}$")]
GitSha1 = Annotated[str, StringConstraints(pattern=r"^[0-9a-f]{40}$")]

class DigestRef(EvalModel):
    algorithm: Literal["sha256"] = "sha256"
    digest: Sha256

class ArtifactDigest(EvalModel):
    id: str
    path: str
    sha256: Sha256

class VerificationDigest(EvalModel):
    command: str
    exit_code: int
    stdout_sha256: Sha256
    stderr_sha256: Sha256
    verified_commit: GitSha1
    passed: bool

class ControllerIntervention(EvalModel):
    sequence: int = Field(ge=0)
    kind: Literal["campaign_init", "manifest_finalize", "product_source_edit"]
    reason: str
    invalidates_campaign: bool

class ForgeProvenanceEntry(EvalModel):
    schema_version: Literal[1] = 1
    sequence: int = Field(ge=1)
    previous_entry_sha256: Sha256 | None
    task_id: str
    run_id: str
    agent: Literal["forge"] = "forge"
    provider: str
    model: str
    transport: str
    input_commit: GitSha1
    patch_sha256: Sha256
    output_commit: GitSha1
    trace_sha256: Sha256
    report_sha256: Sha256
    harness_fingerprint: Sha256
    dataset_fingerprint: Sha256
    verification_digest: Sha256
    trust_status: Literal["trusted"] = "trusted"
    cost_cny: Decimal = Field(ge=0)

class ForgeBuildManifest(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    status: Literal["complete", "invalid"]
    baseline_commit: str
    final_commit: str
    fixture: ArtifactDigest
    development_dataset: ArtifactDigest
    holdout_dataset: ArtifactDigest
    harness_profile: ArtifactDigest
    provenance_ledger: ArtifactDigest
    artifact_index: ArtifactDigest
    final_tree: ArtifactDigest
    final_verification: VerificationDigest
    accepted_tasks: list[str]
    controller_interventions: list[ControllerIntervention]
```

Add validators for sequence/previous-hash consistency at the collection level in the next task, not in a single entry.

**Step 4: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_provenance_models.py -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/provenance_models.py apps/eval-runner/tests/test_provenance_models.py
git commit -m "feat(eval): define Forge provenance schemas"
```

---

### Task 2: Implement canonical artifact hashing and append-only ledger verification

**Files:**

- Add: `apps/eval-runner/app/provenance.py`
- Add: `apps/eval-runner/tests/test_provenance.py`
- Reuse: `apps/eval-runner/app/artifact_hashing.py`

**Step 1: Write failing tests**

Cover:

- canonical JSON produces the same bytes regardless of dictionary insertion order;
- append writes one newline-terminated compact JSON object and fsyncs it;
- the first entry requires `previous_entry_sha256=None`;
- later entries require the previous canonical entry hash;
- duplicate run ID, task ID, patch hash, or output commit is rejected;
- truncation, reordering, replacement, malformed JSON, blank lines, or extra data invalidates the ledger;
- trace/report/patch artifact hashes are recomputed, not trusted from the JSON;
- no artifact contains known API-key patterns.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_provenance.py -q
```

Expected: FAIL.

**Step 3: Implement canonical primitives**

```python
def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")

def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()

def entry_digest(entry: ForgeProvenanceEntry) -> str:
    return sha256_bytes(canonical_json_bytes(entry.model_dump(mode="json")))
```

Append with `os.open(..., O_APPEND | O_CREAT | O_WRONLY, 0o600)`, one `os.write`, and `os.fsync`. Refuse to append until the existing ledger fully verifies.

**Step 4: Implement ledger verification**

Return a structured `LedgerVerification` with:

- `ok`;
- entry count;
- final entry digest;
- blockers;
- ordered task/run/commit IDs.

Check sequence, previous hash, uniqueness, trust status, input/output commit continuity, and referenced artifact digests.

**Step 5: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_provenance.py -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/provenance.py apps/eval-runner/tests/test_provenance.py
git commit -m "feat(eval): verify append-only Forge provenance"
```

---

### Task 3: Initialize an integration campaign from the locked fixture

**Files:**

- Add: `apps/eval-runner/app/campaign.py`
- Add: `apps/eval-runner/tests/test_campaign.py`

**Step 1: Write failing initialization tests**

Assert `initialize_campaign`:

- verifies fixture and dataset locks first;
- copies the fixture to `campaign/worktree`;
- initializes a new Git repository with no remote;
- commits the canonical baseline;
- stores ledger/artifacts outside `worktree`;
- records campaign metadata, controller/runtime versions, and baseline tree digest;
- fails if the fixture digest differs or generated files exist;
- never copies the source fixture's Git metadata.

Expected layout:

```text
campaign/
  campaign.json
  pending-accept.json      # exists only while an accept transaction is recoverable
  ledger.jsonl
  artifacts/
    traces/
    reports/
    patches/
    verification/
  worktree/
    .git/
    ...ForgeBench product...
```

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py -k initialize -q
```

Expected: FAIL.

**Step 3: Implement initialization**

Use argument-list Git commands only:

```python
git(worktree, "init", "-b", "forge-built")
git(worktree, "config", "user.name", "Forge Integration Controller")
git(worktree, "config", "user.email", "forge-integration@local.invalid")
git(worktree, "add", ".")
git(worktree, "commit", "-m", "chore: initialize canonical ForgeBench baseline")
```

Record the actual commit and canonical tree digest. `campaign.json` contains no secrets and has mode `0600`.

**Step 4: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py -k initialize -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/campaign.py apps/eval-runner/tests/test_campaign.py
git commit -m "feat(eval): initialize Forge integration campaigns"
```

---

### Task 4: Accept only trusted Forge scoring patches

**Files:**

- Modify: `apps/eval-runner/app/campaign.py`
- Modify: `apps/eval-runner/tests/test_campaign.py`

**Step 1: Write the rejection matrix**

Acceptance must fail without changing Git or ledger state when:

- report trust is unknown/untrusted;
- task did not pass;
- workspace observation is unavailable or mismatched;
- scope violations exist;
- patch replay is absent or failed;
- trace provider/model/transport differs from the frozen campaign profile;
- dataset or harness fingerprint differs;
- trace/report/patch hashes differ;
- current `HEAD` differs from the declared input commit;
- patch does not apply cleanly;
- targeted/hidden/full verification fails;
- Git shows controller-authored changes before patch application;
- task/run/output commit is duplicated.

Add one happy-path test that accepts a two-file patch, verifies it, commits it, appends one entry, and leaves a clean worktree.

Add crash-injection tests at each transaction boundary. Before a Git commit,
recovery must reverse the explicit patch and remove only named temporary
artifacts. After a Git commit, recovery must verify that exact output commit and
finish the intended artifact rename and ledger append; it must never accept
another task while `pending-accept.json` exists.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py -k accept -q
```

Expected: FAIL.

**Step 3: Implement the acceptance transaction**

Order is mandatory:

1. verify campaign and ledger;
2. refuse a new accept when a pending transaction exists;
3. verify clean worktree and current input commit;
4. load and validate trace/report;
5. recompute patch from `AgentTrace.file_diffs`;
6. compare authoritative observed files to patch files;
7. run `git apply --check`;
8. apply patch;
9. run visible, hidden, and full verification;
10. copy immutable artifacts to transaction-scoped temporary names;
11. fsync `pending-accept.json` with the validated input commit, hashes, and
    intended ledger entry fields;
12. stage only authoritative changed files;
13. commit with trailers constructed exactly as:

```python
message = (
    f"feat(forgebench): accept {entry.task_id}\n\n"
    f"Forge-Run-Id: {entry.run_id}\n"
    f"Forge-Task-Id: {entry.task_id}\n"
    f"Forge-Trace-SHA256: {entry.trace_sha256}\n"
    f"Forge-Patch-SHA256: {entry.patch_sha256}\n"
)
```

14. update and fsync the pending record with the actual output commit;
15. atomically rename the prepared immutable artifacts;
16. append the provenance entry;
17. reverify ledger and clean worktree;
18. remove `pending-accept.json`.

If a failure occurs after patch application but before commit, reverse only the
explicit patch with `git apply -R` and verify the original commit/tree. If the
process stops after commit, `recover_pending_accept` completes only the exact
recorded transaction after verifying commit, artifacts, and hashes. Never use
`git reset --hard`, broad cleanup, or an unproven automatic retry.

**Step 4: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py -k "accept or recover_pending" -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/campaign.py apps/eval-runner/tests/test_campaign.py
git commit -m "feat(eval): integrate trusted Forge patches"
```

---

### Task 5: Finalize and independently verify `FORGE-BUILD-MANIFEST.json`

**Files:**

- Modify: `apps/eval-runner/app/campaign.py`
- Add: `apps/eval-runner/app/campaign_verify.py`
- Modify: `apps/eval-runner/tests/test_campaign.py`
- Add: `apps/eval-runner/tests/test_campaign_verify.py`

**Step 1: Write failing finalization and tamper tests**

Finalization must:

- require a fully valid ledger;
- require every accepted commit to have matching trailers;
- rerun full-monorepo verification from clean `HEAD`;
- write an artifact index listing every immutable trace/report/patch/verification file;
- write `FORGE-BUILD-MANIFEST.json` into the product root;
- commit only that generated manifest as the final controller-declared metadata commit;
- mark product-source controller interventions as invalid.

Verification must fail after independently changing:

- any source byte;
- the manifest;
- ledger line/order;
- trace/report/patch;
- commit trailer/history;
- dataset/fixture/harness lock;
- final verification result.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py -k finalize tests/test_campaign_verify.py -q
```

Expected: FAIL.

**Step 3: Implement finalization**

The final manifest stores digests, not absolute paths. The artifact index uses paths relative to the campaign root. The final verification record contains command, exit code, bounded output digests, duration, and verified commit.

The final tree digest excludes `.git` and excludes `FORGE-BUILD-MANIFEST.json` while the manifest is being constructed; record that exclusion rule as `tree_digest_profile="forgebench-v1"`.

**Step 4: Implement independent verification**

`verify_campaign(campaign_root)` must use only on-disk data and Git; it must not trust in-memory finalization objects. It recomputes:

- fixture/dataset/harness lock digests;
- every ledger entry and link;
- every artifact reference;
- every input/output commit transition;
- each commit patch versus recorded patch;
- final product tree;
- final clean verification.

Return nonzero with stable blocker codes and no secret-bearing output.

**Step 5: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_campaign.py tests/test_campaign_verify.py -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/campaign.py apps/eval-runner/app/campaign_verify.py apps/eval-runner/tests/test_campaign.py apps/eval-runner/tests/test_campaign_verify.py
git commit -m "feat(eval): finalize verifiable Forge builds"
```

---

### Task 6: Expose safe campaign CLI commands

**Files:**

- Modify: `apps/eval-runner/app/cli.py`
- Add: `apps/eval-runner/app/campaign_cli.py`
- Modify: `apps/eval-runner/tests/test_cli.py`

**Step 1: Write failing CLI tests**

Cover:

```bash
python -m app.cli campaign init
python -m app.cli campaign accept
python -m app.cli campaign status
python -m app.cli campaign finalize
python -m app.cli campaign verify
```

Requirements:

- mutating commands require explicit `--campaign`;
- `accept` requires explicit trace/report/patch inputs and never selects “latest”;
- `status` and `verify` are read-only;
- JSON output contains hashes, IDs, trust, and blockers but no raw prompts, environment, or credentials;
- unknown/malformed campaign paths return exit 2;
- trust/tamper failure returns exit 1.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_cli.py -k campaign -q
```

Expected: FAIL.

**Step 3: Add dispatch and parsers**

At the top of `main`, after `promote-trace` and before normal eval parsing:

```python
if raw_argv[:1] == ["campaign"]:
    return campaign_main(raw_argv[1:])
```

Use subparsers with required commands. Paths resolve explicitly and no command changes directories outside the declared campaign/worktree.

**Step 4: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_cli.py -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/cli.py apps/eval-runner/app/campaign_cli.py apps/eval-runner/tests/test_cli.py
git commit -m "feat(eval): add Forge campaign CLI"
```

---

### Task 7: Document and gate machine-verifiable Forge authorship

**Files:**

- Modify: `README.md`
- Modify: `apps/eval-runner/README.md`
- Modify: `CHANGELOG.md`
- Add: `docs/forgebench-provenance.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

**Step 1: Add failing acceptance assertions**

Require a gate named `Forge-built monorepo provenance contract` with:

```text
uv run pytest tests/test_provenance_models.py tests/test_provenance.py tests/test_campaign.py tests/test_campaign_verify.py
```

Also require docs to contain `FORGE-BUILD-MANIFEST.json`, `controller interventions`, and `campaign verify`.

**Step 2: Run and confirm failure**

```bash
node --test scripts/acceptance.test.mjs
```

Expected: FAIL.

**Step 3: Add gate and documentation**

Add:

```bash
add_gate 'Forge-built monorepo provenance contract' 'cd apps/eval-runner && uv run pytest tests/test_provenance_models.py tests/test_provenance.py tests/test_campaign.py tests/test_campaign_verify.py'
```

Document:

- scoring and integration tracks are distinct;
- only trusted passing Forge patches can enter the product commit chain;
- the canonical baseline is a controller-authored benchmark scaffold and is
  identified separately from the Forge-authored patch chain;
- human/controller product edits invalidate the current chain;
- how to initialize, accept, finalize, and verify;
- what the manifest proves and does not prove;
- H0 provides the proof mechanism but does not yet prove holdout success.

**Step 4: Full H0 offline verification**

Run:

```bash
npm run test:eval
cd apps/eval-runner
uv run ruff check app tests
uv run mypy app
uv run python -m app.cli forgebench verify-catalog
cd ../..
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
npm run build:desktop
```

Expected: PASS.

**Step 5: Review GitNexus scope and commit**

Run `detect_changes({scope: "all", base_ref: "main"})`. Verify only provenance, campaign, CLI dispatch, eval docs, and acceptance flows are affected.

```bash
git add README.md apps/eval-runner/README.md CHANGELOG.md docs/forgebench-provenance.md scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "docs(eval): codify verifiable Forge authorship"
```

---

## H0C Exit Criteria

- A clean canonical fixture can initialize a campaign with ledger/artifacts outside the product workspace.
- Only trusted, scope-clean, replayable, verified Forge patches can create product commits.
- Every accepted commit is linked to immutable trace, report, patch, dataset, and harness hashes.
- Tampering with any source, commit, ledger entry, or artifact makes `campaign verify` fail closed.
- `FORGE-BUILD-MANIFEST.json` is generated and independently verifiable.
- No README assertion is treated as authorship evidence.
- No paid DeepSeek call is made in H0C.
