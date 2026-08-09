# Forge H0B Canonical ForgeBench Monorepo and Dataset Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task with test-first commits.

**Goal:** Replace the scattered executable coding fixtures with one coherent, versioned ForgeBench product monorepo and freeze a 10-case development split plus a 20-case holdout split.

**Architecture:** Every real coding task begins with the same canonical `forgebench-monorepo` baseline. A task may add a small versioned scenario overlay, but never points to another project. The loader resolves the baseline and overlay descriptors, hashes their trees rather than absolute paths, and keeps the holdout split sealed from development reports. Case-specific hidden verifiers live outside the agent-visible fixture and are addressed by opaque verifier IDs.

**Tech Stack:** Bun test/build, TypeScript/TSX, Python 3.11, FastAPI-compatible service modules, pytest, JSON case manifests, SHA-256 canonical tree hashing.

**Design reference:** `docs/superpowers/specs/2026-08-07-forge-frontier-deepseek-harness-design.md`, Sections 7.1, 7.4, and Phase H0.

**GitNexus preflight:** `dataset_fingerprint` is MEDIUM risk with 4 direct and 31 total upstream dependents. Introduce the new descriptor fields compatibly, then change fingerprint semantics under explicit tests. Run impact analysis on `EvaluationTask`, `_resolve_fixture_path`, `prepare_workspace`, `dataset_fingerprint`, and `validate_case_quality` immediately before implementation.

---

### Task 1: Define canonical fixture, scenario, split, and verifier fields

**Files:**

- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/cases.py`
- Modify: `apps/eval-runner/tests/test_cases.py`

**Step 1: Write failing loader and quality tests**

Add tests that load one case with:

```json
{
  "id": "forgebench-dev-domain-normalize-title",
  "title": "Normalize task titles",
  "prompt": "Normalize repeated whitespace in task titles.",
  "fixture_id": "forgebench-monorepo",
  "fixture_version": "1.0.0",
  "fixture_path": "../../forgebench-monorepo",
  "scenario_path": "./scenario",
  "dataset_split": "development",
  "hidden_verifier_id": "domain-normalize-title",
  "full_verification_command": "python tools/release/verify.py",
  "validation_commands": ["bun test tests/cases/domain-normalize-title.test.ts"],
  "expected_files_changed": ["packages/domain/src/task-title.ts"]
}
```

Assert:

- relative fixture and scenario paths resolve to absolute paths;
- `dataset_split` validates to an enum;
- missing fixture ID, version, hidden verifier, or full verification command is an error for `metadata.real_coding=true`;
- a holdout task with `metadata.holdout_unsealed=true` is a quality error;
- legacy contract-only tasks remain valid without the new fields.

**Step 2: Run the tests and confirm the failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py -q
```

Expected: FAIL because the descriptor fields and split enum do not exist.

**Step 3: Add the schema**

Add:

```python
class DatasetSplit(StrEnum):
    DEVELOPMENT = "development"
    HOLDOUT = "holdout"

class EvaluationTask(EvalModel):
    # existing fields stay in their current order
    fixture_id: str | None = None
    fixture_version: str | None = None
    fixture_path: str | None = None
    scenario_path: str | None = None
    dataset_split: DatasetSplit | None = None
    hidden_verifier_id: str | None = None
    full_verification_command: str | None = None
```

Extend `_resolve_fixture_path` into `_resolve_case_paths` and resolve both `fixture_path` and `scenario_path` against the case file directory. Preserve `None` and absolute paths unchanged.

In `validate_case_quality`, when `task.metadata.get("real_coding") is True`, require:

```python
required = {
    "fixture_id": task.fixture_id,
    "fixture_version": task.fixture_version,
    "fixture_path": task.fixture_path,
    "dataset_split": task.dataset_split,
    "hidden_verifier_id": task.hidden_verifier_id,
    "full_verification_command": task.full_verification_command,
}
```

Emit stable error codes `missing_fixture_id`, `missing_fixture_version`, `missing_dataset_split`, `missing_hidden_verifier`, and `missing_full_verification`. Validate that both paths exist.

**Step 4: Run the tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/models.py apps/eval-runner/app/cases.py apps/eval-runner/tests/test_cases.py
git commit -m "feat(eval): model canonical benchmark cases"
```

---

### Task 2: Hash fixture and scenario contents instead of machine paths

**Files:**

- Add: `apps/eval-runner/app/artifact_hashing.py`
- Modify: `apps/eval-runner/app/datasets.py`
- Add: `apps/eval-runner/tests/test_artifact_hashing.py`
- Modify: `apps/eval-runner/tests/test_storage.py`

**Step 1: Write the failing hashing tests**

Cover:

- equal trees at two absolute paths have the same digest;
- a content, executable-bit, symlink-target, or relative-path change changes the digest;
- `.git`, `node_modules`, `.venv`, `__pycache__`, `.pytest_cache`, `dist`, and `coverage` are excluded;
- dataset fingerprints are independent of absolute fixture/case locations;
- fixture and scenario content changes alter the dataset fingerprint;
- hidden verifier content changes alter the dataset fingerprint;
- `expected_success`, pass-to-pass, fail-to-pass, full verification, split, and hidden verifier ID all affect the fingerprint.

**Step 2: Run the tests and confirm the failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_artifact_hashing.py tests/test_storage.py -q
```

Expected: FAIL because canonical tree hashing does not exist and fingerprints include absolute paths.

**Step 3: Implement canonical tree hashing**

Create:

```python
IGNORED_TREE_PARTS = frozenset(
    {".git", "node_modules", ".venv", "__pycache__", ".pytest_cache", "dist", "coverage"}
)

def canonical_tree_manifest(root: Path) -> list[dict[str, str | int]]:
    entries: list[dict[str, str | int]] = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if any(part in IGNORED_TREE_PARTS for part in relative.parts):
            continue
        if path.is_dir():
            continue
        if path.is_symlink():
            kind = "symlink"
            content = os.fsencode(os.readlink(path))
        else:
            kind = "file"
            content = path.read_bytes()
        entries.append(
            {
                "path": relative.as_posix(),
                "kind": kind,
                "mode": stat.S_IMODE(path.lstat().st_mode),
                "sha256": hashlib.sha256(content).hexdigest(),
            }
        )
    return entries

def canonical_tree_digest(root: Path) -> str:
    payload = json.dumps(
        canonical_tree_manifest(root),
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return hashlib.sha256(payload).hexdigest()
```

**Step 4: Replace path-dependent dataset hashing**

Build each task payload with:

```python
{
    "id": task.id,
    "title": task.title,
    "prompt": task.prompt,
    "fixture": {
        "id": task.fixture_id,
        "version": task.fixture_version,
        "digest": tree_digest_or_none(task.fixture_path),
    },
    "scenario_digest": tree_digest_or_none(task.scenario_path),
    "dataset_split": task.dataset_split,
    "hidden_verifier_id": task.hidden_verifier_id,
    "hidden_verifier_digest": verifier_digest_or_none(task.hidden_verifier_id),
    "context_files": task.context_files,
    "setup_commands": task.setup_commands,
    "validation_commands": task.validation_commands,
    "post_validation_commands": task.post_validation_commands,
    "pass_to_pass_commands": task.pass_to_pass_commands,
    "fail_to_pass_commands": task.fail_to_pass_commands,
    "verification_command": task.verification_command,
    "full_verification_command": task.full_verification_command,
    "expected_success": task.expected_success,
    "expected_files_changed": task.expected_files_changed,
    "forbidden_files_changed": task.forbidden_files_changed,
    "required_scores": task.required_scores,
    "max_duration_seconds": task.max_duration_seconds,
    "max_model_rounds": task.max_model_rounds,
    "tags": task.tags,
    "metadata": task.metadata,
}
```

`verifier_digest_or_none` resolves only a validated ID beneath
`apps/eval-runner/forgebench/verifiers`, hashes the verifier plus
`_support.py`, and never places their absolute paths in the payload. Serialize
enums in JSON mode. Never hash the resolved absolute fixture or scenario path.

**Step 5: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_artifact_hashing.py tests/test_storage.py -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/artifact_hashing.py apps/eval-runner/app/datasets.py apps/eval-runner/tests/test_artifact_hashing.py apps/eval-runner/tests/test_storage.py
git commit -m "feat(eval): fingerprint benchmark contents"
```

---

### Task 3: Materialize the canonical ForgeBench product monorepo

**Files:**

- Add tree: `apps/eval-runner/forgebench/forgebench-monorepo/`
- Add: `apps/eval-runner/forgebench/fixture-lock.json`
- Add: `apps/eval-runner/app/forgebench.py`
- Add: `apps/eval-runner/tests/test_forgebench_fixture.py`

**Step 1: Write the failing fixture contract test**

The test must assert:

- the directory exists and contains no `.git`;
- the required top-level directories are exactly present;
- `package.json` declares workspaces for both apps and all TS packages;
- `pyproject.toml` includes both Python services;
- `python tools/release/verify.py` exits zero from a clean copy;
- the canonical digest matches `fixture-lock.json`;
- the tree has no generated/cache directories.

**Step 2: Run the test and confirm the failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_fixture.py -q
```

Expected: FAIL because the fixture does not exist.

**Step 3: Create the exact product tree**

Create:

```text
forgebench-monorepo/
  apps/
    web-console/src/{main.tsx,task-list.tsx,saved-filters.ts}
    admin-console/src/{main.tsx,bulk-actions.ts,permissions.ts}
  services/
    task-api/task_api/{__init__.py,app.py,models.py,repository.py,service.py}
    event-worker/event_worker/{__init__.py,consumer.py,retry.py,store.py}
  packages/
    domain/src/{index.ts,task.ts,task-title.ts,task-query.ts,task-sort.ts}
    ui/src/{index.ts,empty-state.tsx,priority-badge.tsx}
    config/src/{index.ts,env.ts,feature-flags.ts}
  tools/release/{verify.py,build-manifest.ts,checksums.ts}
  tests/
    baseline/{domain.test.ts,ui.test.ts,config.test.ts,test_task_api.py,test_worker.py}
    contract/{task-schema.test.ts,event-schema.test.ts}
    integration/{bulk-edit.test.ts,test_task_lifecycle.py}
  package.json
  tsconfig.json
  pyproject.toml
  README.md
```

Use these root scripts:

```json
{
  "name": "forgebench-monorepo",
  "private": true,
  "version": "1.0.0",
  "workspaces": [
    "apps/*",
    "packages/*"
  ],
  "scripts": {
    "test:ts": "bun test tests/baseline/*.test.ts tests/contract/*.test.ts tests/integration/*.test.ts",
    "test:py": "uv run pytest tests/baseline/test_*.py tests/integration/test_*.py -q",
    "typecheck": "out=$(mktemp -d); trap 'rm -rf \"$out\"' EXIT; bun build apps/web-console/src/main.tsx apps/admin-console/src/main.tsx --outdir \"$out\" --external react --external react-dom/client",
    "verify": "python tools/release/verify.py"
  }
}
```

Use a Python `tools/release/verify.py` that executes, in order:

```python
COMMANDS = [
    ["bun", "run", "test:ts"],
    ["uv", "run", "pytest", "tests/baseline/test_task_api.py", "tests/baseline/test_worker.py", "tests/integration/test_task_lifecycle.py", "-q"],
    ["bun", "run", "typecheck"],
    [sys.executable, "-m", "compileall", "-q", "services"],
]
```

The baseline represents one task-management product:

- `packages/domain` owns task identity, status, priority, title normalization, filtering, and sorting;
- both consoles import domain and UI packages;
- `task-api` owns validation and conditional mutation;
- `event-worker` consumes task events with retry and dedupe state;
- release tools verify every consumer and service.

Do not add third-party source vendoring, lockfile-generated caches, nested repositories, solution notes, or benchmark case tests to the default baseline test glob.

**Step 4: Freeze the actual digest**

Implement `freeze_fixture` in `app/forgebench.py`. It writes a JSON object
whose keys are exactly `schema_version`, `fixture_id`, `fixture_version`,
`canonical_tree_sha256`, and `verification_command`; the digest value comes
directly from `canonical_tree_digest(fixture)`.

The implementation command is:

```bash
cd apps/eval-runner
uv run python -m app.forgebench freeze-fixture \
  --fixture forgebench/forgebench-monorepo \
  --output forgebench/fixture-lock.json
```

The command must refuse a dirty/generated fixture and atomically write the computed hash; no digest is typed by hand.

Give `app/forgebench.py` its own `main(argv)` and
`if __name__ == "__main__": raise SystemExit(main())` in this task, so the
freeze command is available before the general eval CLI dispatch is added in
Task 8.

**Step 5: Run fixture verification**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_fixture.py -q
(cd forgebench/forgebench-monorepo && python tools/release/verify.py)
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/forgebench.py apps/eval-runner/forgebench/forgebench-monorepo apps/eval-runner/forgebench/fixture-lock.json apps/eval-runner/tests/test_forgebench_fixture.py
git commit -m "feat(eval): add canonical ForgeBench monorepo"
```

---

### Task 4: Apply versioned scenario overlays without creating another project

**Files:**

- Modify: `apps/eval-runner/app/runner.py`
- Modify: `apps/eval-runner/tests/test_runner.py`

**Step 1: Write failing workspace preparation tests**

Test that `prepare_workspace`:

- copies the canonical fixture;
- overlays a scenario file at the same relative path;
- adds scenario-only files;
- rejects scenario symlinks or paths escaping the workspace;
- produces independent scoring and patch-replay copies;
- never mutates the source fixture or scenario.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -k scenario_overlay -q
```

Expected: FAIL because `prepare_workspace` ignores `scenario_path`.

**Step 3: Implement safe overlays**

Add:

```python
def apply_scenario_overlay(scenario: Path, workspace: Path) -> None:
    for source in sorted(scenario.rglob("*")):
        relative = source.relative_to(scenario)
        if source.is_symlink():
            raise ValueError(f"scenario symlinks are forbidden: {relative.as_posix()}")
        target = workspace / relative
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
```

Call it immediately after `shutil.copytree(fixture, workspace)`. Patch replay uses the same `prepare_workspace`, so it receives the identical scenario baseline.

**Step 4: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -k "scenario_overlay or patch_replay" -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/runner.py apps/eval-runner/tests/test_runner.py
git commit -m "feat(eval): support ForgeBench scenario overlays"
```

---

### Task 5: Add controller-owned hidden verifier execution

**Files:**

- Add: `apps/eval-runner/app/hidden_verifiers.py`
- Modify: `apps/eval-runner/app/runner.py`
- Add tree: `apps/eval-runner/forgebench/verifiers/`
- Add: `apps/eval-runner/tests/test_hidden_verifiers.py`
- Modify: `apps/eval-runner/tests/test_runner.py`

**Step 1: Write failing tests**

Cover:

- unknown verifier ID fails closed;
- verifier IDs reject `/`, `..`, and whitespace-only values;
- a verifier runs with the workspace path as an argument;
- verifier source is never included in the payload sent to the agent command;
- verifier failure becomes `verification_failed`;
- full-monorepo verification runs after the hidden verifier and blocks pass on regression;
- timeout/cancellation are preserved.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_hidden_verifiers.py tests/test_runner.py -k "hidden_verifier or full_verification" -q
```

Expected: FAIL.

**Step 3: Implement opaque verifier IDs**

Use a fixed root:

```python
VERIFIER_ROOT = Path(__file__).resolve().parents[1] / "forgebench" / "verifiers"

def verifier_command(verifier_id: str, workspace: Path) -> list[str]:
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", verifier_id):
        raise ValueError("invalid hidden verifier id")
    path = VERIFIER_ROOT / f"{verifier_id}.py"
    if not path.is_file():
        raise ValueError(f"unknown hidden verifier: {verifier_id}")
    return [sys.executable, str(path), str(workspace)]
```

Run the command after visible validation and before `full_verification_command`. Do not add either value to the Rust headless task payload:

```python
task_dict.pop("hidden_verifier_id", None)
task_dict.pop("full_verification_command", None)
task_dict.pop("scenario_path", None)
```

Record verifier ID, command digest, exit code, duration, and bounded stdout/stderr in controller-owned shell evidence.

**Step 4: Add verifier shared library**

Create `forgebench/verifiers/_support.py` with:

- workspace containment checks;
- dynamic TS test execution via `bun test`;
- Python test execution via `uv run pytest`;
- JSON stdout result;
- 12,000-character output cap;
- no writes to the workspace.

Each public verifier file imports `_support.py`, defines one exact behavior assertion, and exits non-zero on mismatch.

**Step 5: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_hidden_verifiers.py tests/test_runner.py -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/hidden_verifiers.py apps/eval-runner/app/runner.py apps/eval-runner/forgebench/verifiers apps/eval-runner/tests/test_hidden_verifiers.py apps/eval-runner/tests/test_runner.py
git commit -m "feat(eval): add controller-owned hidden verification"
```

---

### Task 6: Freeze the 10 development cases

**Files:**

- Add tree: `apps/eval-runner/forgebench/cases/development/`
- Add or modify matching files: `apps/eval-runner/forgebench/verifiers/*.py`
- Add: `apps/eval-runner/tests/test_forgebench_catalog.py`

**Step 1: Write the split contract test**

Assert that the development split contains exactly these IDs and no holdout task:

| ID | Task kind | Allowed source paths | Targeted verification |
|---|---|---|---|
| `forgebench-dev-domain-normalize-title` | TS bug fix | `packages/domain/src/task-title.ts` | repeated Unicode whitespace normalizes once |
| `forgebench-dev-web-filter-query` | frontend fix | `apps/web-console/src/task-list.tsx`, `packages/domain/src/task-query.ts` | quoted status/search query filters correctly |
| `forgebench-dev-admin-bulk-status` | multi-file feature | `apps/admin-console/src/bulk-actions.ts`, `packages/domain/src/task.ts` | bulk transition rejects invalid state atomically |
| `forgebench-dev-task-api-bulk-update` | Python feature | `services/task-api/task_api/service.py`, `repository.py` | mixed valid/invalid bulk update rolls back |
| `forgebench-dev-worker-retry-backoff` | Python bug fix | `services/event-worker/event_worker/retry.py` | capped exponential retry has no off-by-one |
| `forgebench-dev-ui-priority-badge` | TSX accessibility | `packages/ui/src/priority-badge.tsx` | label and class match every priority |
| `forgebench-dev-config-env-parser` | config fix | `packages/config/src/env.ts` | blank, nonnumeric, non-canonical, and out-of-range ports fail |
| `forgebench-dev-release-artifact-manifest` | shell-heavy tooling | `tools/release/build-manifest.ts` | sorted files and SHA-256 output are reproducible |
| `forgebench-dev-contract-event-schema` | cross-service contract | `packages/domain/src/task.ts`, `services/event-worker/event_worker/consumer.py` | v2 optional fields preserve v1 consumers |
| `forgebench-dev-idempotent-no-op` | correct no-change | no source path | existing behavior is proven and workspace stays clean |

Every case must declare:

```json
{
  "fixture_id": "forgebench-monorepo",
  "fixture_version": "1.0.0",
  "fixture_path": "../../../forgebench-monorepo",
  "scenario_path": "./scenario",
  "dataset_split": "development",
  "metadata": {
    "real_coding": true,
    "holdout_unsealed": false
  },
  "forbidden_files_changed": [
    "package.json",
    "pyproject.toml",
    "tools/release/verify.py",
    ".forge/**"
  ],
  "required_scores": ["task_success", "scope_adherence"],
  "max_duration_seconds": 900,
  "max_model_rounds": 40
}
```

The no-op case has no scenario overlay, `expected_files_changed=[]`, and a hidden verifier that confirms the requested behavior already exists.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_catalog.py -k development -q
```

Expected: FAIL because the split is absent.

**Step 3: Add exact case directories**

Each directory contains only:

```text
case.json
scenario/       # omitted for the no-op task
```

Scenario overlays introduce the requested bug or missing behavior without embedding a solution, answer comment, future commit, hidden assertion, or alternate implementation. Visible validation commands may exercise public behavior, while the hidden verifier covers edge cases.

Each case must run:

1. one targeted visible command;
2. its hidden verifier;
3. `python tools/release/verify.py`.

**Step 4: Validate baseline and scenario polarity**

Add catalog checks that:

- the canonical baseline passes full verification;
- each development scenario fails its hidden verifier before an agent edit, except the declared no-op;
- each scenario still passes the unrelated baseline suite before an agent edit;
- every allowed/expected path exists or has an existing parent;
- no scenario changes root package/dependency files.

**Step 5: Run tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_catalog.py -k development -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/forgebench/cases/development apps/eval-runner/forgebench/verifiers apps/eval-runner/tests/test_forgebench_catalog.py
git commit -m "test(eval): freeze ForgeBench development split"
```

---

### Task 7: Freeze and seal the 20 holdout cases

**Files:**

- Add tree: `apps/eval-runner/forgebench/cases/holdout/`
- Add or modify matching files: `apps/eval-runner/forgebench/verifiers/*.py`
- Modify: `apps/eval-runner/tests/test_forgebench_catalog.py`

**Step 1: Extend the catalog test with the exact holdout inventory**

| ID | Task kind | Primary scope |
|---|---|---|
| `forgebench-holdout-domain-due-date-zone` | TS date bug | domain due-date normalization |
| `forgebench-holdout-domain-stable-sort` | TS algorithm fix | stable multi-key sorting |
| `forgebench-holdout-web-saved-filters` | frontend feature | saved-filter serialization |
| `forgebench-holdout-web-optimistic-rollback` | frontend diagnosis | failed mutation rollback |
| `forgebench-holdout-admin-pagination-query` | frontend bug | query/page reset behavior |
| `forgebench-holdout-admin-permission-guard` | security fix | bulk action authorization |
| `forgebench-holdout-api-validation` | Python validation | title/status/priority contract |
| `forgebench-holdout-api-conditional-update` | API feature | revision/ETag conflict |
| `forgebench-holdout-worker-quarantine` | worker recovery | poison message quarantine |
| `forgebench-holdout-worker-dedupe` | worker correctness | duplicate event idempotency |
| `forgebench-holdout-ui-empty-state-escape` | TSX security | untrusted label escaping |
| `forgebench-holdout-ui-keyboard-shortcut` | UI behavior | editable-target shortcut guard |
| `forgebench-holdout-config-port-range` | config fix | valid TCP port bounds |
| `forgebench-holdout-config-feature-flag` | config fix | strict boolean parsing |
| `forgebench-holdout-release-version-match` | release tooling | package/service version parity |
| `forgebench-holdout-release-checksums` | release tooling | deterministic bundle checksums |
| `forgebench-holdout-contract-task-forward-compat` | contract fix | unknown optional JSON fields |
| `forgebench-holdout-integration-bulk-edit` | cross-layer feature | admin → API bulk edit |
| `forgebench-holdout-ambiguous-notification-badge` | repository inspection | infer unread-count source |
| `forgebench-holdout-long-context-audit-log` | long-context multi-file | API event through worker and consoles |

Assert exactly 20 holdout tasks, unique IDs, no development overlap, one declared long-context task, at least six Python-involved tasks, at least eight TypeScript-involved tasks, at least five multi-file tasks, at least four tempting forbidden-path cases, and no mock metadata.

**Step 2: Run and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_catalog.py -k holdout -q
```

Expected: FAIL because the holdout split is absent.

**Step 3: Add holdout case directories**

Use the same case contract as development, except:

```json
{
  "dataset_split": "holdout",
  "metadata": {
    "real_coding": true,
    "holdout_unsealed": false,
    "development_visible": false
  }
}
```

Set `max_model_rounds=48` only on `forgebench-holdout-long-context-audit-log`; all other tasks use 40 or fewer. Its later experiment profile may receive the declared 2 CNY cap; the case metadata does not contain provider pricing.

Holdout prompts state user intent, not target filenames. Context files are empty unless the user would naturally provide one. Hidden verifiers must not share descriptive filenames with implementation symbols.

**Step 4: Seal the split**

Implement:

```bash
cd apps/eval-runner
uv run python -m app.forgebench freeze-dataset \
  --cases forgebench/cases \
  --fixture-lock forgebench/fixture-lock.json \
  --output forgebench/dataset-lock.json
```

`dataset-lock.json` contains schema version, fixture identity/digest, development fingerprint/count/IDs, holdout fingerprint/count/IDs, combined fingerprint, and verifier-tree digest. It is atomically generated from actual contents.

**Step 5: Run catalog polarity and leak checks**

```bash
cd apps/eval-runner
uv run pytest tests/test_forgebench_catalog.py -q
uv run python -m app.forgebench verify-catalog \
  --cases forgebench/cases \
  --lock forgebench/dataset-lock.json
```

Expected: PASS. The command must not print holdout prompts or verifier contents; it prints counts and hashes only.

**Step 6: Commit**

```bash
git add apps/eval-runner/forgebench/cases/holdout apps/eval-runner/forgebench/verifiers apps/eval-runner/forgebench/dataset-lock.json apps/eval-runner/tests/test_forgebench_catalog.py
git commit -m "test(eval): freeze sealed ForgeBench holdout"
```

---

### Task 8: Route real-coding defaults to ForgeBench without deleting contract cases

**Files:**

- Modify: `apps/eval-runner/app/cli.py`
- Modify: `apps/eval-runner/tests/test_cli.py`
- Modify: `apps/eval-runner/README.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

**Step 1: Write failing CLI tests**

Add subcommand coverage:

```bash
python -m app.cli forgebench verify-catalog
python -m app.cli forgebench show-lock --split development
python -m app.cli forgebench show-lock --split holdout
```

The holdout command prints only IDs/count/fingerprint until H4 explicitly unseals a campaign; it never prints prompts.

Assert the legacy `--cases` interface still works for contract/runtime cases.

**Step 2: Implement the `forgebench` CLI dispatch**

At the top of `main`:

```python
if raw_argv[:1] == ["forgebench"]:
    return forgebench_main(raw_argv[1:])
```

Keep `promote-trace` and normal eval behavior unchanged. `forgebench_main` lives in `app/forgebench.py`, owns fixture/dataset freeze and verification, and accepts explicit paths for tests.

**Step 3: Add the acceptance gate**

Add:

```bash
add_gate 'ForgeBench canonical monorepo and frozen dataset' 'cd apps/eval-runner && uv run pytest tests/test_artifact_hashing.py tests/test_forgebench_fixture.py tests/test_forgebench_catalog.py tests/test_hidden_verifiers.py && uv run python -m app.cli forgebench verify-catalog'
```

Update `scripts/acceptance.test.mjs` to require the label and command.

**Step 4: Update docs**

State precisely:

- `eval_cases/` remains for Forge runtime/contract tests;
- every executable real coding benchmark now targets one `forgebench-monorepo`;
- the old `continuity-ts-tooling` and standalone micro fixtures are excluded
  from frontier scoring and retained only until their runtime-contract
  assertions are migrated; they can never satisfy `metadata.real_coding=true`;
- scoring attempts use independent copies and scenario overlays;
- 10 development and 20 sealed holdout cases are content-fingerprinted;
- H0 does not claim paid-task success.

**Step 5: Verify**

```bash
cd apps/eval-runner
uv run pytest -q
uv run ruff check app tests
uv run mypy app
uv run python -m app.cli forgebench verify-catalog
cd ../..
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Expected: PASS.

**Step 6: Review GitNexus scope and commit**

Run `detect_changes({scope: "all", base_ref: "main"})`. Verify that the only changed execution paths are case loading, workspace preparation, verification, fingerprints, CLI dispatch, and acceptance.

```bash
git add apps/eval-runner/app/cli.py apps/eval-runner/app/forgebench.py apps/eval-runner/tests/test_cli.py apps/eval-runner/README.md README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "docs(eval): make ForgeBench the real coding benchmark"
```

---

## H0B Exit Criteria

- One canonical ForgeBench monorepo passes its full clean baseline verification.
- All 30 executable real coding tasks point to that fixture and no other project.
- Catalog validation fails if any `metadata.real_coding=true` task points to a
  different fixture ID or path; runtime/contract fixtures are not treated as
  competing benchmark projects.
- The development split has exactly 10 cases and holdout has exactly 20.
- Absolute filesystem locations do not affect fixture, scenario, verifier, or dataset fingerprints.
- Every non-no-op scenario fails its hidden verifier before repair while unrelated baseline checks pass.
- The frozen holdout fingerprint is committed but no holdout prompt has been used for harness tuning.
- No paid DeepSeek call is made in H0B.
