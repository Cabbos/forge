# Forge H0A Eval Isolation and Golden Truth Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task with test-first commits.

**Goal:** Make Forge evaluation independent of the user's desktop state and replace the mock-only harness check with an offline observer/patch-replay self-check.

**Architecture:** `ForgeAgentRunner` owns an ephemeral run root containing two siblings: the observed product workspace and unobserved runtime state. It starts the headless Forge command with an isolated `HOME`/`USERPROFILE`, always supplies the isolated Continuity database, and emits non-secret runtime-isolation evidence. The top-level doctor precheck uses the same clean-home rule. The golden harness gate combines the existing deterministic contract check with a real filesystem-observer and patch-replay check.

**Tech Stack:** Python 3.11, Pydantic 2, pytest, Node.js test runner, Bun CLI doctor, Git patch replay.

**Design reference:** `docs/superpowers/specs/2026-08-07-forge-frontier-deepseek-harness-design.md`, Phase H0.

**GitNexus preflight:** Already measured on 2026-08-07. `ForgeAgentRunner.run_task`, `run_golden_harness_check`, and the doctor wrapper are LOW risk. `run_bounded_process` is MEDIUM risk with 8 direct callers and one affected eval execution flow. Preserve its default behavior when `env` is omitted.

---

### Task 1: Run the eval doctor against an isolated home

**Files:**

- Modify: `scripts/eval-doctor-precheck.mjs`
- Modify: `scripts/eval-doctor-precheck.test.mjs`

**Step 1: Write the failing tests**

Add imports for `existsSync` and a new exported `runDoctorIsolated` helper. Add these tests:

```js
test("doctor precheck runs against a disposable Forge home", () => {
  let captured;
  const result = runDoctorIsolated({
    spawn(command, args, options) {
      captured = { command, args, options };
      assert.equal(existsSync(options.env.HOME), true);
      assert.equal(existsSync(join(options.env.HOME, ".forge")), true);
      return { stdout: '{"ok":true,"checks":[]}', stderr: "", status: 0 };
    },
  });

  assert.equal(captured.command, "bun");
  assert.deepEqual(captured.args.slice(-2), ["doctor", "--json"]);
  assert.equal(captured.options.env.HOME, captured.options.env.USERPROFILE);
  assert.equal(existsSync(result.isolatedHome), false);
});

test("a corrupted user snapshot cannot enter the isolated doctor report", () => {
  const userHome = mkdtempSync(join(tmpdir(), "forge-user-home-"));
  mkdirSync(join(userHome, ".forge", "sessions"), { recursive: true });
  writeFileSync(join(userHome, ".forge", "sessions", "broken.json"), "{");
  let evaluatedHome;

  try {
    runDoctorIsolated({
      baseEnv: { ...process.env, HOME: userHome, USERPROFILE: userHome },
      spawn(_command, _args, options) {
        evaluatedHome = options.env.HOME;
        assert.notEqual(evaluatedHome, userHome);
        assert.equal(existsSync(join(evaluatedHome, ".forge", "sessions", "broken.json")), false);
        return { stdout: '{"ok":true,"checks":[]}', stderr: "", status: 0 };
      },
    });
  } finally {
    rmSync(userHome, { recursive: true, force: true });
  }
});
```

Also update the test imports:

```js
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  classifyDoctorReport,
  exitCodeForDoctorReport,
  runDoctorIsolated,
} from "./eval-doctor-precheck.mjs";
```

**Step 2: Run the tests and confirm the failure**

Run:

```bash
node --test scripts/eval-doctor-precheck.test.mjs
```

Expected: FAIL because `runDoctorIsolated` is not exported.

**Step 3: Implement disposable-home doctor execution**

Replace `runDoctor()` with the following helper pair:

```js
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export function runDoctorIsolated({
  spawn = spawnSync,
  baseEnv = process.env,
} = {}) {
  const isolatedHome = mkdtempSync(join(tmpdir(), "forge-eval-doctor-"));
  mkdirSync(join(isolatedHome, ".forge"), { recursive: true });
  try {
    const result = spawn("bun", ["run", cliEntry, "doctor", "--json"], {
      cwd: repoRoot,
      env: {
        ...baseEnv,
        HOME: isolatedHome,
        USERPROFILE: isolatedHome,
      },
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 30_000,
    });
    return {
      stdout: result.stdout ?? "",
      stderr: result.stderr ?? "",
      status: result.status,
      isolatedHome,
    };
  } finally {
    rmSync(isolatedHome, { recursive: true, force: true });
  }
}

export function runDoctor() {
  const { stdout, stderr, status } = runDoctorIsolated();
  return { stdout, stderr, status };
}
```

Keep `classifyDoctorReport`, `exitCodeForDoctorReport`, and `main` behavior unchanged. The disposable `.forge` directory makes `forge_logs` a clean-install pass without weakening real repo, Bun, Cargo, or eval-runner checks.

**Step 4: Run the test**

Run:

```bash
node --test scripts/eval-doctor-precheck.test.mjs
```

Expected: PASS.

**Step 5: Commit**

```bash
git add scripts/eval-doctor-precheck.mjs scripts/eval-doctor-precheck.test.mjs
git commit -m "fix(eval): isolate doctor precheck state"
```

---

### Task 2: Allow bounded child processes to receive an explicit environment

**Files:**

- Modify: `apps/eval-runner/app/process_control.py`
- Modify: `apps/eval-runner/tests/test_process_control.py`

**Step 1: Write the failing test**

```python
def test_bounded_process_accepts_an_explicit_environment(tmp_path: Path) -> None:
    env = os.environ.copy()
    env["FORGE_EVAL_CHILD_SENTINEL"] = "isolated"

    result = run_bounded_process(
        [sys.executable, "-c", "import os; print(os.environ['FORGE_EVAL_CHILD_SENTINEL'])"],
        cwd=tmp_path,
        timeout_seconds=2,
        env=env,
    )

    assert result.returncode == 0
    assert result.stdout.strip() == "isolated"
```

Add `import os`.

**Step 2: Run the test and confirm the failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_process_control.py::test_bounded_process_accepts_an_explicit_environment -q
```

Expected: FAIL because `run_bounded_process` does not accept `env`.

**Step 3: Add the backwards-compatible parameter**

```python
from collections.abc import Callable, Mapping, Sequence

def run_bounded_process(
    command: str | Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    input_text: str | None = None,
    shell: bool = False,
    env: Mapping[str, str] | None = None,
) -> BoundedProcessResult:
    started = time.monotonic()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        shell=shell,
        text=True,
        stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
        env=dict(env) if env is not None else None,
    )
```

Do not merge the supplied mapping inside `run_bounded_process`; callers must construct the complete environment explicitly. This keeps all existing callers byte-for-byte equivalent when `env=None`.

**Step 4: Run focused and full process tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_process_control.py -q
```

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/eval-runner/app/process_control.py apps/eval-runner/tests/test_process_control.py
git commit -m "feat(eval): support isolated child environments"
```

---

### Task 3: Isolate every Forge task runtime from the observed workspace and user home

**Files:**

- Modify: `apps/eval-runner/app/runner.py`
- Modify: `apps/eval-runner/tests/test_runner.py`

**Step 1: Tighten the existing isolation test**

Rename `test_forge_runner_isolates_forbidden_runtime_state_from_workspace` to `test_forge_runner_isolates_all_runtime_state_from_workspace_and_user_home`.

Remove the conditional nature of the assertion by changing the fake agent body to:

```python
"runtime_state=pathlib.Path(payload['runtime_state_path'])\n"
"continuity_db=pathlib.Path(payload['continuity_database_path'])\n"
"runtime_home=pathlib.Path(os.environ['HOME'])\n"
"assert os.environ['USERPROFILE'] == str(runtime_home)\n"
"assert runtime_state.parent == workspace.parent\n"
"assert runtime_state != workspace and workspace not in runtime_state.parents\n"
"assert continuity_db == runtime_state/'continuity.db'\n"
"assert runtime_home == runtime_state/'home'\n"
"assert runtime_home != pathlib.Path(sys.argv[1])\n"
"(runtime_home/'.forge'/'sessions').mkdir(parents=True,exist_ok=True)\n"
"(runtime_home/'.forge'/'sessions'/'eval.json').write_text('{}')\n"
```

Add `import os` to the generated script, use
`command=[sys.executable, str(script), str(Path.home())]`, and remove
`.forge/continuity.db` from `forbidden_files_changed`. The test must still
observe exactly `["src/app.py"]`, no scope violations, and successful patch
replay.

Add an assertion that the raw events include:

```python
assert {
    "event_type": "eval_runtime_isolation",
    "runtime_state": "runtime-state",
    "runtime_home": "runtime-state/home",
    "continuity_database": "runtime-state/continuity.db",
} in trace.raw_events
```

**Step 2: Run the test and confirm the failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py::test_forge_runner_isolates_all_runtime_state_from_workspace_and_user_home -q
```

Expected: FAIL because Continuity is still conditional and the child inherits the user home.

**Step 3: Implement universal runtime isolation**

Inside `ForgeAgentRunner.run_task`, replace the conditional Continuity path with:

```python
runtime_state = temp_root / "runtime-state"
runtime_home = runtime_state / "home"
runtime_home.mkdir(parents=True, exist_ok=True)
continuity_database_path = runtime_state / "continuity.db"
child_env = os.environ.copy()
child_env.update(
    {
        "HOME": str(runtime_home),
        "USERPROFILE": str(runtime_home),
        "FORGE_EVAL_RUNTIME_STATE": str(runtime_state),
    }
)
```

Always include:

```python
payload["continuity_database_path"] = str(continuity_database_path)
```

Pass `env=child_env` only to the headless Forge `run_bounded_process` call. Setup and verification commands remain controller-owned and retain their existing environment.

After `_trace_from_payload`, add non-secret evidence:

```python
isolation_event = {
    "event_type": "eval_runtime_isolation",
    "runtime_state": "runtime-state",
    "runtime_home": "runtime-state/home",
    "continuity_database": "runtime-state/continuity.db",
}
trace = trace.model_copy(update={"raw_events": [*trace.raw_events, isolation_event]})
```

Import `os`. Delete `FORGE_MANAGED_RUNTIME_PATHS` and simplify
`scope_violations_for` to treat every unexpected path equally:

```python
if expected and path not in expected:
    violations.append(f"unexpected_change:{path}")
```

Add a unit assertion that a reported `.forge/continuity.db` workspace change is
now an `unexpected_change` unless it is explicitly expected. Runtime files are
outside the workspace, so no observer exemption remains valid.

**Step 4: Add a no-edit isolation regression**

Add:

```python
def test_forge_runner_runtime_files_never_become_changed_files(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    script = tmp_path / "runtime_only_agent.py"
    script.write_text(
        "import json,os,pathlib,sys\n"
        "home=pathlib.Path(os.environ['HOME'])\n"
        "(home/'.forge'/'sessions').mkdir(parents=True,exist_ok=True)\n"
        "(home/'.forge'/'sessions'/'one.json').write_text('{}')\n"
        "json.dump({'final_answer':'no source change','changed_files':[]},sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="runtime-only",
        title="Runtime only",
        prompt="Inspect only.",
        fixture_path=str(fixture),
        validation_commands=[f'{sys.executable} -c "print(1)"'],
        expected_files_changed=[],
    )

    trace = ForgeAgentRunner(command=[sys.executable, str(script)]).run_task(task)

    assert trace.changed_files == []
    assert trace.scope_violations == []
```

**Step 5: Run runner tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -q
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/runner.py apps/eval-runner/tests/test_runner.py
git commit -m "fix(eval): isolate every Forge task runtime"
```

---

### Task 4: Replace the mock-only golden gate with real observer and replay evidence

**Files:**

- Modify: `apps/eval-runner/app/harness_checks.py`
- Modify: `apps/eval-runner/tests/test_execution.py`
- Modify: `apps/eval-runner/tests/test_workspace_observer.py`

**Step 1: Write the self-check tests**

In `tests/test_workspace_observer.py`:

```python
def test_observer_patch_replay_self_check_passes() -> None:
    result = run_observer_patch_replay_self_check()

    assert result.ok is True
    assert result.untracked_files == []
    assert result.modified_files == ["src/value.txt"]
    assert result.message == "observer and patch replay self-check passed"
```

In `tests/test_execution.py`, monkeypatch `run_observer_patch_replay_self_check` to return `WorkspaceCheck(ok=False, message="replay mismatch")` and assert the execution trust blockers contain `harness_check_failed`.

**Step 2: Run the tests and confirm the failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_workspace_observer.py tests/test_execution.py -q
```

Expected: FAIL because the self-check does not exist and the golden gate does not call it.

**Step 3: Implement the offline self-check**

Add to `app/harness_checks.py`:

```python
import tempfile

from app.models import FileDiff
from app.patches import replay_patch
from app.workspace_observer import observe_workspace_changes, snapshot_workspace

GOLDEN_PATCH = """diff --git a/src/value.txt b/src/value.txt
--- a/src/value.txt
+++ b/src/value.txt
@@ -1 +1 @@
-before
+after
"""

def run_observer_patch_replay_self_check() -> WorkspaceCheck:
    with tempfile.TemporaryDirectory(prefix="forge-harness-golden-") as raw_root:
        root = Path(raw_root)
        workspace = root / "workspace"
        replay = root / "replay"
        for target in (workspace, replay):
            (target / "src").mkdir(parents=True)
            (target / "src" / "value.txt").write_text("before\n", encoding="utf-8")

        before = snapshot_workspace(workspace)
        (workspace / "src" / "value.txt").write_text("after\n", encoding="utf-8")
        observed = observe_workspace_changes(
            before,
            workspace,
            reported_changed_files=["src/value.txt"],
        )
        replayed = replay_patch(
            replay,
            [FileDiff(path="src/value.txt", diff=GOLDEN_PATCH)],
            timeout_seconds=10,
        )
        replay_value = (replay / "src" / "value.txt").read_text(encoding="utf-8")
        ok = (
            observed.available
            and observed.changed_files == ["src/value.txt"]
            and observed.mismatch_files == []
            and replayed.ok
            and replay_value == "after\n"
        )
        return WorkspaceCheck(
            ok=ok,
            untracked_files=observed.mismatch_files,
            modified_files=observed.changed_files,
            message=(
                "observer and patch replay self-check passed"
                if ok
                else "observer or patch replay self-check failed"
            ),
        )
```

In `run_golden_harness_check`, run this after the deterministic traces. If it fails, return its failure. If both checks pass, return:

```python
return WorkspaceCheck(
    ok=True,
    message="deterministic contract, observer, and patch replay checks passed",
)
```

**Step 4: Run focused tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_workspace_observer.py tests/test_execution.py -q
```

Expected: PASS.

**Step 5: Run the full eval suite**

Run:

```bash
cd apps/eval-runner
uv run pytest -q
```

Expected: PASS, including with a corrupted snapshot under the real user `~/.forge/sessions`.

**Step 6: Commit**

```bash
git add apps/eval-runner/app/harness_checks.py apps/eval-runner/tests/test_execution.py apps/eval-runner/tests/test_workspace_observer.py
git commit -m "test(eval): make golden harness evidence real"
```

---

### Task 5: Document and gate the isolation contract

**Files:**

- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `apps/eval-runner/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

**Step 1: Write the failing acceptance assertions**

Add assertions to `scripts/acceptance.test.mjs` that the acceptance matrix contains exactly one gate named `eval runtime isolation and golden truth`, and that its command contains:

```text
node --test scripts/eval-doctor-precheck.test.mjs
uv run pytest tests/test_process_control.py tests/test_runner.py tests/test_workspace_observer.py tests/test_execution.py
```

**Step 2: Run the contract test and confirm the failure**

Run:

```bash
node --test scripts/acceptance.test.mjs
```

Expected: FAIL because the gate is absent.

**Step 3: Add the acceptance gate**

In the eval acceptance domain, immediately before `eval runner test suite`, add:

```bash
add_gate 'eval runtime isolation and golden truth' 'node --test scripts/eval-doctor-precheck.test.mjs && cd apps/eval-runner && uv run pytest tests/test_process_control.py tests/test_runner.py tests/test_workspace_observer.py tests/test_execution.py'
```

**Step 4: Update documentation**

Document the following exact contract in all three READMEs and the changelog:

- eval prechecks use a disposable home and never inspect user session snapshots;
- each real Forge attempt uses sibling `workspace` and `runtime-state` roots;
- `HOME`, `USERPROFILE`, registry, Continuity, logs, profiles, and snapshots are routed outside the observed product workspace;
- trusted harness status requires a real observer plus patch-replay self-check.

Do not claim the 20-case frontier holdout has passed; this is H0 evidence only.

**Step 5: Run documentation and acceptance checks**

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
npm run test:eval
```

Expected: all PASS. The last command must no longer depend on the health of user snapshots.

**Step 6: Run formatting and static checks**

Run:

```bash
cd apps/eval-runner
uv run ruff check app tests
uv run mypy app
```

Expected: PASS.

**Step 7: Review GitNexus scope and commit**

Run `detect_changes({scope: "all", base_ref: "main"})`. Confirm only eval precheck, process execution, Forge runner isolation, golden checks, docs, and acceptance flows are affected.

```bash
git add README.md apps/desktop/README.md apps/eval-runner/README.md CHANGELOG.md scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "docs(eval): codify isolated harness truth"
```

---

## H0A Exit Criteria

- `npm run test:eval` passes even when the user's desktop session store contains corrupt JSON.
- Every real Forge child receives a disposable home under the run's unobserved runtime root.
- Continuity, registry, profiles, logs, snapshots, and other home-derived state cannot appear in authoritative workspace changes.
- The golden trust gate exercises the real filesystem observer and real `git apply` replay path.
- No paid DeepSeek call is made in H0A.
