# Forge DeepSeek Chat Completions A/B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fail-closed, eval-only OpenAI Chat Completions route for DeepSeek V4 Flash and produce a machine-verifiable paired comparison against the Anthropic control without changing desktop defaults.

**Architecture:** A canonical Python harness profile selects one of two official DeepSeek routes. `ForgeAgentRunner` writes a secret-free one-provider settings file under the child process's isolated runtime home and sends the same route as typed expected identity in the headless request. Rust loads the effective profile through the existing settings path and requires an exact provider/model/transport/base-URL match before preflight or session creation. Shared desktop session construction remains unchanged. Both campaign arms reuse the immutable baseline ledger and trust gates, and a separate comparison artifact verifies that transport is the only experimental difference.

**Tech Stack:** Rust/Tokio/Reqwest/Serde, Python 3.11/Pydantic/Pytest, canonical JSON/SHA-256, Forge headless eval protocol.

**Current execution status (2026-08-08):** Tasks 1–6 are implemented and
offline-verified. A synchronized-source paid diagnostic also exposed and then
closed a cache-boundary false positive: Bun/uv cache roots now live under
isolated runtime state, while only the explicit `.bun-cache` disposable
directory is excluded by both observers. Task 7 is intentionally open:
paid diagnostics have produced trusted route smokes but have also hit the
controller's unknown-cost and patch-replay trust boundaries, so no complete
30-attempt arm or paired comparison is claimed. The first post-fix pair reached
six trusted attempts per arm before the same unknown-cost multi-file API
boundary, which the controller recovered without replay. Offline reproduction
then found a separate artifact-publication false positive: source-level
`api_key = os.getenv(...)` was classified as secret material. The scrubber now
requires token-shaped `sk-...` or Bearer values, with regression coverage;
historical paid artifacts remain immutable and are not replayed. H4 holdout
execution remains closed until a complete trusted development pair exists.

---

### Task 1: Generalize the frozen harness profile to two official routes

**Files:**
- Modify: `apps/eval-runner/app/harness_profiles.py`
- Modify: `apps/eval-runner/app/baseline_models.py`
- Create: `apps/eval-runner/forgebench/profiles/deepseek-v4-flash-openai-chat-control-v1.json`
- Modify: `apps/eval-runner/tests/test_harness_profiles.py`
- Modify: `apps/eval-runner/tests/test_baseline.py`

- [ ] **Step 1: Run GitNexus impact analysis**

Run impact analysis for `HarnessProfileV1`, `load_harness_profile`, and `BaselineReport`. Record direct callers and stop if any result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing profile-route tests**

Add tests that load both committed profiles, require canonical bytes, and reject crossed endpoint/transport pairs:

```python
@pytest.mark.parametrize(
    ("transport", "endpoint_family", "base_url"),
    [
        ("anthropic_messages", "anthropic", "https://api.deepseek.com/anthropic"),
        ("openai_chat_completions", "openai", "https://api.deepseek.com"),
    ],
)
def test_profile_accepts_only_official_deepseek_routes(
    profile_payload: dict[str, object],
    transport: str,
    endpoint_family: str,
    base_url: str,
) -> None:
    payload = {
        **profile_payload,
        "profile_id": f"deepseek-v4-flash-{endpoint_family}-control-v1",
        "transport": transport,
        "endpoint_family": endpoint_family,
        "base_url": base_url,
    }
    assert HarnessProfileV1.model_validate(payload).transport == transport
```

Add negative cases for wrong provider, model, HTTP URL, Responses transport,
Anthropic transport with the OpenAI URL, and OpenAI transport with the
Anthropic URL.

- [ ] **Step 3: Run the tests and observe RED**

Run:

```bash
UV_CACHE_DIR=/private/tmp/forge-uv-cache PYTHONDONTWRITEBYTECODE=1 \
uv run pytest -q -p no:cacheprovider tests/test_harness_profiles.py tests/test_baseline.py
```

Expected: candidate profile and route-pair assertions fail because the schema is still Anthropic-literal.

- [ ] **Step 4: Implement the constrained multi-profile schema**

Replace the literal profile id/transport/endpoint fields with constrained
strings and a cross-field route validator. Add the frozen base URL:

```python
OFFICIAL_ROUTES = {
    ("anthropic_messages", "anthropic"): "https://api.deepseek.com/anthropic",
    ("openai_chat_completions", "openai"): "https://api.deepseek.com",
}

class HarnessProfileV1(EvalModel):
    profile_id: str
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    endpoint_family: Literal["anthropic", "openai"]
    base_url: AnyHttpUrl

    @model_validator(mode="after")
    def validate_official_route(self) -> "HarnessProfileV1":
        expected = OFFICIAL_ROUTES.get((self.transport, self.endpoint_family))
        if expected is None or str(self.base_url) != expected:
            raise ValueError("profile route is not an official frozen DeepSeek route")
        return self
```

Keep every existing identity, split, repetition, price, and budget invariant.
Generalize `BaselineReport.transport` to the same two-value literal. Add
`base_url` to both profile JSON files and write the candidate file in canonical
single-line JSON with one final newline. All non-route experiment fields must
match the control.

- [ ] **Step 5: Run profile and baseline tests**

Run the Step 3 command. Expected: PASS.

### Task 2: Add the typed eval-only route override in Rust

**Files:**
- Modify: `apps/desktop/src-tauri/src/eval_headless/types.rs`
- Modify: `apps/desktop/src-tauri/src/eval_headless/resolve.rs`
- Modify: `apps/desktop/src-tauri/src/eval_headless/mod.rs`

- [ ] **Step 1: Run GitNexus impact analysis**

Run upstream impact for `EvalHeadlessRequest` and `run_request`. Record the
known result that `BuildAgentSessionRequest` and
`build_agent_session_with_registry_path` are CRITICAL/HIGH shared surfaces and
must not be edited for this phase.

- [ ] **Step 2: Write failing Rust request and routing tests**

Add tests that deserialize both official routes and reject:

```rust
serde_json::json!({
    "provider": "deepseek",
    "model": "deepseek-v4-flash",
    "provider_route": {
        "transport": "openai_chat_completions",
        "base_url": "https://api.deepseek.com"
    },
    "workspace_path": workspace
})
```

Negative cases must cover `openai_responses`, an HTTP URL, a foreign host, a
wrong path, a wrong provider, and a wrong model. Add headless tests that write
the isolated settings file and prove the actual loaded profile must equal the
typed expected route. Preserve an ordinary-default test proving the built-in
desktop DeepSeek profile remains Anthropic.

- [ ] **Step 3: Run the focused Rust tests and observe RED**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  eval_headless:: --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  adapters:: --lib
```

Expected: route types and session override do not exist.

- [ ] **Step 4: Implement the strict route type**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalProviderTransport {
    AnthropicMessages,
    OpenAiChatCompletions,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct EvalProviderRoute {
    pub transport: EvalProviderTransport,
    pub base_url: String,
}
```

Add `provider_route: Option<EvalProviderRoute>` to `EvalHeadlessRequest`.
Implement `validate_expected_provider_route` in `resolve.rs`. It must accept
only the two exact tuples from the design. In `run_request`, load the configured
provider profiles through `settings::load_configured_provider_profiles()`,
resolve the effective `deepseek` profile, and require its transport, base URL,
and default model to equal the typed expected route before credential preflight
or session creation.

The legacy H1 Anthropic profile intentionally omits `base_url` to preserve its
locked canonical bytes and fingerprint. Rust resolves that missing field from
the official `(anthropic_messages, anthropic)` tuple before comparing the
effective route; the new candidate profile carries the explicit OpenAI URL.

- [ ] **Step 5: Preserve shared session construction unchanged**

Do not change `BuildAgentSessionRequest`,
`build_agent_session_with_registry_path`, or any IPC/session-lifecycle wrapper.
GitNexus classifies those shared surfaces as CRITICAL/HIGH because they fan out
through desktop IPC, session lifecycle, permission-harness, and eval flows.

The Python runner writes a canonical, secret-free provider entry to
`<runtime_home>/.forge/config.json` before starting the process. Existing
credential resolution and session construction naturally read the same
isolated file after Rust has attested it. The entry contains
`api_key_env: "DEEPSEEK_API_KEY"` and never contains an API key value.

Credential preflight must validate the route before returning and include
`transport` plus `endpoint_family` in its zero-call response.

- [ ] **Step 6: Run focused tests**

Run both commands from Step 3. Expected: PASS.

### Task 3: Propagate route identity through the Python controller

**Files:**
- Modify: `apps/eval-runner/app/runner.py`
- Modify: `apps/eval-runner/app/baseline_cli.py`
- Modify: `apps/eval-runner/app/baseline.py`
- Modify: `apps/eval-runner/tests/test_runner.py`
- Modify: `apps/eval-runner/tests/test_baseline_cli.py`
- Modify: `apps/eval-runner/tests/test_baseline.py`

- [ ] **Step 1: Run GitNexus impact analysis**

Run upstream impact for `ForgeAgentRunner`, `credential_preflight`, `run_task`,
and `_attempt_blockers`.

- [ ] **Step 2: Write failing payload and identity tests**

Assert that the candidate runner sends:

```python
"provider_route": {
    "transport": "openai_chat_completions",
    "base_url": "https://api.deepseek.com",
}
```

Assert that it also writes canonical `<runtime_home>/.forge/config.json` with
the selected transport/base URL/model and API-key environment variable name,
contains no secret value, and leaves the user's desktop settings untouched.
Assert that credential preflight rejects a returned transport or endpoint
family that differs from the profile. Assert that `_attempt_blockers` requires
provider, model, transport, endpoint family, and harness fingerprint to match.

- [ ] **Step 3: Run tests and observe RED**

Run:

```bash
UV_CACHE_DIR=/private/tmp/forge-uv-cache PYTHONDONTWRITEBYTECODE=1 \
uv run pytest -q -p no:cacheprovider \
  tests/test_runner.py tests/test_baseline_cli.py tests/test_baseline.py
```

- [ ] **Step 4: Implement route propagation**

Extend `ForgeCredentialPreflight` and `ForgeAgentRunner` with required transport,
endpoint-family, and base-URL identity. Add the same `provider_route` to
preflight and run payloads. Add `endpoint_family` to the
`eval_agent_identity` event. In `baseline_cli._run`, construct the runner only
from the loaded canonical profile and require the exact preflight identity
before `run_baseline_campaign`.

- [ ] **Step 5: Run the focused Python tests**

Run the Step 3 command. Expected: PASS.

### Task 4: Prove Chat Completions conformance and usage accounting

**Files:**
- Modify: `apps/desktop/src-tauri/src/adapters/openai_compatible.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/provider_conformance.rs`
- Modify: `apps/eval-runner/tests/test_provider_usage.py`

- [ ] **Step 1: Run GitNexus impact analysis**

Run upstream impact for `OpenAiCompatibleAdapter.call_non_streaming`,
`OpenAiCompatibleAdapter.stream_message_with_emitter`, and
`account_provider_usage`.

- [ ] **Step 2: Add conformance fixtures before changing parser code**

Cover text-only streaming, one tool call, two parallel tool calls, malformed
arguments, inclusive prompt/cache usage, error JSON, SSE keep-alive comments,
cancellation, and a stream ending before `[DONE]`. Reuse the existing local
mock-server test pattern. Do not add DeepSeek-specific parsing unless a fixture
fails under the standard Chat Completions contract.

- [ ] **Step 3: Run adapter tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  adapters::openai_compatible --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  adapters::provider_conformance --lib
```

- [ ] **Step 4: Make only fixture-proven parser fixes**

Preserve tool-call ids, merge fragmented arguments by tool index, ignore
keep-alive comments, emit one canonical usage event, and return a typed error
for malformed or truncated terminal state. Keep source `openai` and provider id
`deepseek` distinct in usage evidence.

- [ ] **Step 5: Prove Python inclusive accounting remains intact**

Add an OpenAI-source test with `input_tokens=2_400` and
`cache_read_tokens=2_000`; assert total input `2_400`, hit `2_000`, miss `400`,
and known CNY cost. Run:

```bash
UV_CACHE_DIR=/private/tmp/forge-uv-cache PYTHONDONTWRITEBYTECODE=1 \
uv run pytest -q -p no:cacheprovider tests/test_provider_usage.py
```

### Task 5: Add a fail-closed paired comparison artifact

**Files:**
- Create: `apps/eval-runner/app/baseline_comparison.py`
- Modify: `apps/eval-runner/app/baseline_models.py`
- Modify: `apps/eval-runner/app/baseline_cli.py`
- Create: `apps/eval-runner/tests/test_baseline_comparison.py`
- Modify: `apps/eval-runner/tests/test_baseline_cli.py`

- [ ] **Step 1: Run GitNexus impact analysis**

Run upstream impact for `baseline_main`, `BaselineReport`, and
`verify_baseline_campaign`.

- [ ] **Step 2: Write failing comparison tests**

Create two complete synthetic campaign reports with identical source, dataset,
model, price table, task/repetition keys, and non-route profile fields. Assert
that comparison succeeds only when transport/endpoint/profile id/fingerprint
differ. Reject incomplete, blocked, untrusted, mismatched-order, mismatched
budget-policy, and same-transport pairs.

- [ ] **Step 3: Define the comparison model**

The model must include:

```python
class BaselineComparison(EvalModel):
    schema_version: Literal[1] = 1
    control_campaign_id: str
    candidate_campaign_id: str
    dataset_fingerprint: Sha256
    model: Literal["deepseek-v4-flash"]
    control_transport: Literal["anthropic_messages"]
    candidate_transport: Literal["openai_chat_completions"]
    paired_attempts: Literal[30]
    outcome_changes: dict[str, str]
    control_metrics: BaselineComparisonMetrics
    candidate_metrics: BaselineComparisonMetrics
    winner: Literal["control", "candidate", "no_material_difference"]
    blockers: list[str]
```

Metrics include trusted success, verification and scope rates, median/p95
rounds, median/p95 cost and duration, failure taxonomy, provider errors, and
trust blockers. Canonical JSON plus SHA-256 artifact indexes are required.

- [ ] **Step 4: Add `baseline compare`**

Add explicit `--control-campaign`, `--control-profile`,
`--candidate-campaign`, `--candidate-profile`, and `--output` arguments. The
command independently verifies both campaigns before writing the canonical
comparison. It never chooses “latest”.

- [ ] **Step 5: Run comparison and CLI tests**

Run:

```bash
UV_CACHE_DIR=/private/tmp/forge-uv-cache PYTHONDONTWRITEBYTECODE=1 \
uv run pytest -q -p no:cacheprovider \
  tests/test_baseline_comparison.py tests/test_baseline_cli.py
```

### Task 6: Synchronize acceptance and documentation

**Files:**
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `apps/eval-runner/README.md`
- Modify: `docs/forgebench-h1-baseline.md`
- Create: `docs/forgebench-h2-transport-ab.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Extend the acceptance contract**

Add a “DeepSeek H2 official transport A/B contract” gate containing the focused
Rust route/adapter tests and Python profile/runner/comparison tests. Update the
acceptance matrix contract test with the exact advertised commands.

- [ ] **Step 2: Correct all protocol claims**

Document Chat Completions versus Anthropic, the eval-only route override, the
fact that product defaults do not change, exact paid stop conditions, and the
absence of a DeepSeek Responses claim.

- [ ] **Step 3: Run offline acceptance**

Run:

```bash
scripts/acceptance.sh --only "DeepSeek H2 official transport A/B contract"
```

Expected: all Rust and Python gates pass.

### Task 7: Verify, freeze, and run paid A/B

**Files:**
- Generated outside Git tracking:
  `apps/eval-runner/artifacts/h1-baseline/anthropic-control-v4/`
- Generated outside Git tracking:
  `apps/eval-runner/artifacts/h2-transport/openai-chat-candidate-v1/`
- Generated outside Git tracking:
  `apps/eval-runner/artifacts/h2-transport/comparison-v1.json`

- [ ] **Step 1: Run full offline verification**

Run:

```bash
UV_CACHE_DIR=/private/tmp/forge-uv-cache PYTHONDONTWRITEBYTECODE=1 \
uv run pytest -q -p no:cacheprovider
RUFF_CACHE_DIR=/private/tmp/forge-ruff-cache uv run ruff check .
RUFF_CACHE_DIR=/private/tmp/forge-ruff-cache uv run ruff format --check .
MYPY_CACHE_DIR=/private/tmp/forge-mypy-cache uv run mypy app
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
scripts/acceptance.sh --dry-run
```

- [ ] **Step 2: Run GitNexus `detect_changes` and commit**

Review every affected process, exclude unrelated user files, and commit the H2
implementation, profiles, tests, plans, specs, and documentation.

- [ ] **Step 3: Build the headless binary from the frozen commit**

Build `forge_eval_agent` from the commit that the campaigns will record. Confirm
the paid worktree has no tracked dirt and that `.env` is ignored with mode
`600`.

- [ ] **Step 4: Complete the Anthropic control**

Initialize a new immutable `anthropic-control-v4` campaign. Run one paid smoke,
verify it, then run the remaining attempts. Stop automatically on any blocker;
never retry a terminal or interrupted paid attempt.

- [ ] **Step 5: Run the Chat Completions candidate**

Only after offline conformance and the control are trusted, initialize the
candidate. Run one paid smoke, verify its route and usage identity, then run
the remaining attempts under the H2 allocation.

- [ ] **Step 6: Generate and independently verify the comparison**

Run `baseline report` and `baseline verify` for both arms, then
`baseline compare`. Recompute the comparison SHA-256 and publish exact spend,
trusted success, scope, rounds, cost, latency, blockers, and the selected
transport. Do not claim frontier-grade status; that remains gated on H3 and the
frozen H4 holdout.
