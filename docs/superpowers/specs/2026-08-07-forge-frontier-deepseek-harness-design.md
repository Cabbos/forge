# Forge Frontier DeepSeek Harness Design

> Date: 2026-08-07
> Status: Proposed for implementation
> Scope: `apps/desktop` agent runtime and `apps/eval-runner`
> Target model: official `deepseek-v4-flash`

## 1. Outcome

Forge will turn its existing safety, recovery, and evidence control plane into a
frontier-grade coding-agent harness for DeepSeek V4 Flash. The work is complete
only when the harness demonstrates repeatable real-task gains, not when a new
transport or another set of contract tests merely exists.

The first release gate is a frozen 20-case real coding holdout, executed three
times per case with the selected harness profile:

- trusted success rate is at least 80%;
- changed-file scope violation rate is at most 2%;
- median model-round count is at most 12;
- score and trace coverage is 100% for every required evidence field;
- every run has authoritative workspace evidence, replayable patch evidence,
  verification evidence, usage accounting, and a terminal failure category;
- the entire 60-run holdout stays within its declared DeepSeek budget.

These thresholds are a convergence target, not a benchmark claim about the
underlying model. Forge may call the harness frontier-grade only after the
holdout gate passes from a clean checkout with a frozen dataset fingerprint,
harness fingerprint, model identifier, and provider transport.

## 2. Current Baseline

Forge already has the difficult control-plane foundations:

- an independent multi-round `AgentSession` loop with loop guards;
- parallel read and delegated work plus sequential mutation tools;
- workspace boundaries, permission modes, replayable decisions, and human
  gates;
- session snapshots, append-only loop journals, recovery projections, and
  typed completion evidence;
- a headless runner, independent workspace observation, patch replay, repeated
  trials, layered scorers, red-team cases, and fail-closed trust gates;
- 56 eval cases and a 78-gate acceptance matrix.

The current repository also exposes the gap this design must close:

- recent six-task real Forge reports range from 0% to 50% success;
- failures mix actual model/loop failures with harness pollution such as
  `.forge/continuity.db`, package caches, and generated test files;
- `npm run test:eval` is currently blocked by one corrupt user session snapshot
  even though the eval-runner's 252 Python tests pass independently;
- the built-in DeepSeek profile defaults to V4 Flash over the Anthropic
  Messages-compatible endpoint;
- `ProviderTransport::OpenAiResponses` is represented in configuration and
  probes but does not have a runnable `AiAdapter` implementation;
- the true macOS Tauri force-quit/reopen harness remains unavailable.

The control plane is therefore ahead of the execution plane. This project
focuses on execution truth, task completion, and transport-native behavior.

## 3. Design Principles

### 3.1 Evidence before optimization

No prompt, loop, context, or transport change is retained because it looks
better in one trace. A candidate change must beat the pinned baseline on the
development split without reducing trust coverage, safety, or verification
quality.

### 3.2 One variable per experiment

Each experiment profile changes one named dimension where practical:

- transport;
- reasoning effort;
- system-prompt mode;
- context-selection policy;
- maximum model rounds;
- stagnation policy;
- tool-result presentation;
- verification and repair policy.

The experiment artifact records all dimensions, so an apparent improvement
cannot be attributed to an undocumented configuration difference.

### 3.3 Model-native, protocol-independent runtime

`AgentSession` and the tool executor remain protocol-independent. Chat
Completions, Anthropic Messages, and Responses translate the same internal
message, tool, usage, cancellation, and error contracts through separate
adapters. No provider-specific response item should leak into session logic.

### 3.4 Minimal execution context

The measured minimal profile contains only information needed to solve and
verify the current task. Product memory, continuity, project wiki, skills, and
connectors are individually opt-in experiment dimensions instead of silently
entering the baseline. Stable shared prefixes are ordered before
task-specific content to improve provider cache reuse.

### 3.5 Real effects are authoritative

The harness never trusts the agent's changed-file list, final answer, or
claimed test result as the source of truth. Filesystem snapshots, command exit
codes, patch replay, and verifier outputs remain authoritative.

### 3.6 Holdout integrity

Failed holdout traces may be classified, but they may not drive another
implementation change during the same release attempt. Improvements use the
development split; a modified harness receives a new fingerprint and a fresh
holdout attempt.

## 4. Considered Approaches

### Approach A: Reliability fixes on the current Anthropic transport

This is the shortest path to better numbers. It would isolate runtime state,
clean workspace observation, simplify context, and tune the current loop while
leaving DeepSeek transport unchanged.

Trade-off: it gives fast signal but cannot determine whether Forge is leaving
V4 Flash's Responses-native agent behavior unused. It also risks tuning the
whole harness around one compatibility endpoint.

### Approach B: Responses-first adapter rewrite

This would implement the official Responses protocol immediately and make it
the new default before repairing the eval environment.

Trade-off: transport defects, workspace pollution, and loop defects would be
mixed together. API budget would be spent diagnosing several moving layers,
and a regression could not be assigned confidently.

### Approach C: Evidence-led dual-track convergence

This is the selected approach.

1. Restore harness trust and isolated state without spending model budget.
2. Freeze a baseline dataset and run the existing Anthropic path.
3. Implement a separate Responses adapter behind an experiment profile.
4. Run paired A/B trials with identical task, model, budget, and tool policy.
5. Optimize the winning transport through one-variable experiments.
6. Freeze the winner and run the untouched holdout gate.

This route is slower than a single prompt tweak but produces reusable evidence
and protects the 100 CNY budget from blind iteration.

## 5. Architecture

### 5.1 Experiment profile

Add a versioned harness experiment profile consumed by the headless runner and
stored verbatim in every run artifact.

Conceptual shape:

```json
{
  "schema_version": 1,
  "profile_id": "deepseek-v4-flash-responses-minimal-v1",
  "provider": "deepseek",
  "model": "deepseek-v4-flash",
  "transport": "openai_responses",
  "reasoning_effort": "high",
  "prompt_mode": "minimal",
  "context_policy": "task_local_v1",
  "tool_result_policy": "bounded_structured_v1",
  "parallel_read_tools": true,
  "max_model_rounds": 20,
  "stagnation_policy": "semantic_progress_v1",
  "verification_policy": "required_targeted_then_full_v1",
  "repair_attempt_limit": 1,
  "max_cost_cny_per_task": 1.0
}
```

The canonical JSON hash becomes `harness_fingerprint`. Reports group and
compare only runs with exact matching fingerprints unless a comparison
explicitly names the differing field.

### 5.2 DeepSeek Responses adapter

Create a dedicated Responses adapter implementing the existing `AiAdapter`
trait. It must not be implemented as conditional branches spread across
`openai_compatible.rs`.

Responsibilities:

- translate internal chat messages into Responses input items;
- translate Forge tool definitions into Responses tools;
- preserve tool call identifiers and tool-result adjacency;
- support streaming text, reasoning summaries when present, and parallel tool
  calls;
- normalize usage, cache-hit input, output, and unknown pricing fields into the
  existing provider usage evidence;
- map provider errors into the existing retry/recovery taxonomy;
- honor cancellation without leaving an untracked in-flight request;
- expose conformance fixtures for text-only, one tool call, parallel tool calls,
  malformed arguments, usage, provider error, cancellation, and truncated
  streams.

Provider routing selects the adapter from `ProviderTransport`; it does not infer
transport from the model name. The current Anthropic-compatible DeepSeek
adapter remains available as the control arm until the holdout proves a winner.

### 5.3 Minimal prompt and context mode

The minimal mode will replace the broad generic coding-agent prompt in measured
runs with a compact contract:

1. understand the requested outcome and explicit scope;
2. inspect relevant source before editing;
3. make the smallest sufficient change;
4. run targeted verification and then required final verification;
5. report result, changed files, verification, and unresolved blockers
   truthfully.

The model does not receive prose copies of state already represented by tools
or typed context. Context assembly emits a manifest containing the byte/token
size, origin, priority, and hash of every injected bucket. Duplicate hashes and
overlapping project instructions are rejected or recorded as an explicit
context conflict before the provider call.

The context policy has three measured tiers:

- `task_local`: task, scoped project instructions, selected files, and tool
  contract;
- `project_retrieval`: `task_local` plus ranked project records;
- `continuity`: `project_retrieval` plus eligible memory/continuity candidates.

The first frontier gate uses `task_local`. Other tiers must prove incremental
benefit before becoming defaults.

### 5.4 Progress and stagnation policy

Round count alone is an inadequate loop guard. Each round records typed
progress signals:

- new relevant file inspected;
- new hypothesis formed;
- new patch effect;
- verification command executed;
- verification result improved or regressed;
- blocker resolved;
- user or permission input requested;
- repeated tool call with equivalent arguments;
- repeated error category;
- semantically unchanged assistant plan.

The stagnation policy stops or redirects a run when two consecutive rounds
produce no new progress signal, while preserving exceptions for long-running
commands and explicit human gates. A redirect injects one bounded recovery
instruction containing the observed repetition and the next required evidence.
Only one redirect is allowed before terminal `no_progress`.

The policy distinguishes:

- budget exhaustion;
- tool-loop repetition;
- verification-loop repetition;
- context overflow;
- provider retry exhaustion;
- waiting for permission;
- genuine task completion.

### 5.5 Tool-result contract

Every tool result returned to the model uses a bounded, typed envelope:

```json
{
  "status": "ok",
  "summary": "2 matches in 1 file",
  "facts": {
    "paths": ["src/parser.ts"],
    "exit_code": null,
    "truncated": false
  },
  "content": "bounded task-relevant content",
  "next_evidence": null
}
```

Large shell output, source files, and diffs are stored in artifacts and replaced
in-context with a digest plus a tool for retrieving a bounded slice. Truncation
is explicit. Error envelopes identify whether retrying the same call could
change the result.

Read-only calls may execute in parallel. Mutating calls remain serialized
unless a future design proves isolated worktree ownership for each mutation.

### 5.6 Verification and repair

An executable case declares required verification commands and expected file
effects before the observation window opens. The harness:

1. records the pre-agent workspace snapshot after fixture setup;
2. lets the agent inspect, edit, and run its own targeted checks;
3. runs the independent verifier after the agent reaches a terminal state;
4. permits one repair attempt only when the verifier produces actionable
   failure evidence and budget remains;
5. re-runs the verifier from a fresh process;
6. replays the emitted patch in a fresh fixture to prove portability.

Agent-run tests are useful trajectory evidence but never substitute for the
independent verifier.

### 5.7 Runtime-state and workspace isolation

Each eval attempt receives separate absolute roots for:

- fixture workspace;
- Forge runtime state;
- session snapshots;
- memory/continuity databases;
- logs;
- provider cache metadata;
- eval artifacts.

No eval preflight reads the user's normal Forge session store. A corrupted user
snapshot remains a desktop diagnostics failure but cannot block an isolated
eval run.

Fixture setup, dependency installation, and generated build caches complete
before the authoritative pre-agent snapshot. The observer compares the
post-agent workspace against that snapshot. Runtime state is outside the
fixture, so `.forge/continuity.db` cannot appear as a source edit.

Generated files inside the fixture are not globally ignored. Each case declares
allowed ephemeral patterns, and the report lists them separately from source
effects. An undeclared generated path remains a scope violation.

## 6. Evidence and Failure Taxonomy

Every task attempt must terminate in exactly one primary category:

- `passed`;
- `model_incorrect`;
- `model_incomplete`;
- `model_no_progress`;
- `model_budget_exhausted`;
- `provider_error`;
- `tool_error`;
- `permission_blocked`;
- `verification_failed`;
- `scope_violation`;
- `workspace_observer_untrusted`;
- `patch_replay_failed`;
- `harness_contract_error`;
- `cancelled`;
- `timed_out`.

Secondary findings may coexist, but report aggregation never hides the primary
cause behind a generic `runner_error`.

Required run identity:

- git commit and dirty-state digest;
- dataset and case fingerprints;
- harness profile and fingerprint;
- provider, exact model, endpoint family, and transport;
- temperature, reasoning effort, and output limits;
- tool catalog hash;
- environment/runtime versions;
- attempt and repetition identifiers;
- start/end time, token usage, cache usage, and cost;
- ordered raw events and normalized trajectory;
- authoritative changed files and normalized patch;
- verifier and patch replay evidence;
- trust result and all blockers.

## 7. ForgeBench Monorepo, Dataset, and Experiment Design

### 7.1 One canonical benchmark monorepo

All executable coding cases use one canonical product fixture named
`forgebench-monorepo`. The benchmark must not be a collection of unrelated toy
repositories.

Proposed shape:

```text
forgebench-monorepo/
  apps/
    web-console/          # React/TypeScript product surface
    admin-console/        # second consumer of shared packages
  services/
    task-api/             # Python/FastAPI service
    event-worker/         # background processing and retry behavior
  packages/
    domain/               # shared TypeScript domain rules
    ui/                   # shared components
    config/               # lint/build/test configuration
  tools/
    release/              # build, validation, and packaging scripts
  tests/
    contract/             # cross-app API and schema checks
    integration/          # product-level flows
  package.json
  pyproject.toml
  README.md
```

The monorepo represents one coherent task-management product. Cases span
frontend, backend, shared domain code, background work, release tooling, and
cross-app contracts. This gives Forge realistic navigation, dependency impact,
and verification work instead of one-file benchmark tricks.

The fixture contains no nested `.git` directory. The eval harness copies it to
an isolated workspace, initializes a fresh repository, and records a canonical
baseline tree digest. Every case identifies:

- the fixture version;
- the canonical baseline digest;
- its allowed and forbidden paths;
- setup commands;
- targeted verification;
- full-monorepo verification;
- hidden expected behavior.

All scored attempts begin from the same declared baseline or an explicitly
versioned scenario baseline. They never reuse the mutable workspace from a
previous attempt.

### 7.2 Two execution tracks

The benchmark has two related but separate tracks.

**Scoring track**

- every case starts from a clean isolated copy/worktree;
- task order cannot reveal a previous solution;
- repeated trials are independent;
- headline success, scope, rounds, cost, and trust metrics come only from this
  track.

**Forge-built integration track**

- starts from the canonical monorepo baseline;
- accepts only patches from trusted passing Forge scoring attempts;
- applies accepted patches in a declared sequence;
- runs targeted and full-monorepo verification after every accepted patch;
- records an output commit for each accepted task;
- never supplies controller-authored source edits to make a patch pass;
- stops on patch conflict, verifier regression, or missing provenance.

The scoring track protects experimental validity. The integration track
produces the user-visible proof: a working monorepo whose accepted feature and
fix commits were generated and verified through Forge.

### 7.3 Machine-verifiable Forge provenance

Every trusted integration commit adds one entry to an append-only provenance
artifact outside the agent-visible workspace:

```json
{
  "schema_version": 1,
  "task_id": "forgebench-task-api-bulk-update",
  "run_id": "run-...",
  "agent": "forge",
  "provider": "deepseek",
  "model": "deepseek-v4-flash",
  "transport": "openai_responses",
  "input_commit": "...",
  "patch_sha256": "...",
  "output_commit": "...",
  "trace_sha256": "...",
  "harness_fingerprint": "...",
  "dataset_fingerprint": "...",
  "verification_digest": "...",
  "trust_status": "trusted",
  "cost_cny": 0.0
}
```

The campaign finalizer produces:

- the final ForgeBench monorepo tree or bundle;
- `FORGE-BUILD-MANIFEST.json`;
- the ordered provenance ledger;
- the corresponding trace and report artifact index;
- a verifier result for every accepted commit;
- a final clean full-monorepo build/test result.

The manifest hashes the ledger, final tree, dataset, harness profile, and every
referenced trace. A verification command recomputes those hashes and fails if a
patch, trace, result, or commit was replaced. This is evidence that Forge
generated the accepted code; a README assertion alone is insufficient.

Human/controller changes are allowed only in benchmark infrastructure,
verifiers, harness code, and case metadata. They are never mixed into the
Forge-built product commit chain. If a human changes product source, the chain
starts a new campaign with that intervention declared in the manifest.

### 7.4 Dataset

Create a 30-case executable real-task pack against the canonical monorepo:

- 10 development cases for diagnosis and iteration;
- 20 frozen holdout cases for release decisions.

The pack must include:

- targeted TypeScript and Python fixes;
- multi-file implementation;
- failing-test diagnosis;
- configuration and dependency-safe edits;
- shell-heavy verification;
- ambiguous prompts requiring repository inspection;
- tasks with tempting forbidden changes;
- one long-context repository task;
- one task that should correctly make no source change.

Cases must be solvable, independently verified, scoped, fingerprinted, and free
of future-state hints.

### 7.5 Trial rules

- Development comparisons use at least three repetitions per arm.
- Paired arms use the same task ordering and concurrency.
- A trial with unknown harness trust does not count as a pass and blocks the
  comparison.
- Improvements require a higher trusted success rate or an equal success rate
  with a statistically meaningful reduction in scope violations, rounds, cost,
  or latency.
- Safety and trust regressions are never traded for task success.
- Holdout tasks are run only after the candidate profile is frozen.

### 7.6 Report

The comparison report presents:

- paired outcome changes per task;
- aggregate trusted success and verification rates;
- scope violations;
- median and p95 model rounds;
- median and p95 cost and duration;
- tool call distribution and repeated-call rate;
- failure taxonomy deltas;
- context bucket sizes and duplication findings;
- transport-specific errors;
- confidence interval or bootstrap range for headline deltas;
- all trust blockers and missing evidence.

The report separates scoring-track metrics from integration-track campaign
status. A clean final integration build does not retroactively turn a failed
scoring attempt into a pass.

## 8. Budget Contract

The user-provided DeepSeek budget is 100 CNY. It is divided as follows:

- 8 CNY: baseline calibration and provider conformance smokes;
- 32 CNY: transport A/B and development experiments;
- 20 CNY: loop, context, and verification convergence;
- 35 CNY: frozen 60-run holdout;
- 5 CNY: retry reserve.

The allocation may move between development buckets, but the holdout and retry
reserve may not be consumed by open-ended exploration.

Each run has a per-task cost cap and the experiment has a cumulative hard stop.
The default cap is 1 CNY; a declared long-context case may raise its cap to 2
CNY while the batch remains inside its allocation. Provider usage is
authoritative when available. Missing or contradictory usage sets cost trust to
unknown and blocks a release claim. The system never logs or stores API keys in
an experiment artifact.

Because provider pricing can change, the price table is versioned in the
experiment snapshot and refreshed before a paid batch. Budget decisions use the
price recorded at batch start.

## 9. Delivery Phases

### Phase H0: Restore harness truth

- isolate eval runtime roots from user desktop data;
- classify and remove runtime-state and setup pollution;
- make the top-level eval precheck target isolated eval state;
- add golden observer and patch replay self-checks;
- build and freeze the canonical ForgeBench monorepo fixture;
- freeze the 30-case dataset and fingerprints;
- define the provenance ledger and campaign manifest schema.

Exit: the full mock/golden suite is trusted, and an agent that makes only the
expected edit produces no scope violation.

### Phase H1: Pin the paid baseline

- define the Anthropic-compatible V4 Flash baseline profile;
- run the 10 development cases with three repetitions;
- publish a trusted baseline report and failure distribution.

Exit: every attempt is attributable even if the task fails.

### Phase H2: Native Responses transport

- implement the Responses adapter and fixtures;
- extend probe and conformance coverage;
- add experiment-profile routing without changing product defaults;
- run paired development A/B.

Exit: protocol conformance passes and the A/B report names a winning transport
or proves no material difference.

### Phase H3: Minimal harness convergence

- add minimal context mode;
- add typed bounded tool-result envelopes;
- add progress-based stagnation detection;
- tighten verification and single-repair behavior;
- retain only improvements proven on the development split.

Exit: the chosen candidate meets the target thresholds on development trials.

### Phase H4: Frozen holdout

- freeze commit, dataset fingerprint, harness fingerprint, and price table;
- run 20 holdout cases three times;
- create the Forge-built integration campaign from trusted passing attempts;
- verify the final provenance chain and full monorepo;
- produce the release confidence report;
- run desktop build, backend, eval, protocol, security, and runtime acceptance
  gates.

Exit: all headline thresholds and trust requirements pass inside budget.

## 10. Testing Strategy

### Unit and contract tests

- experiment profile canonicalization and hashing;
- budget accounting and hard stops;
- Responses request/stream parsing fixtures;
- parallel tool calls and tool-result adjacency;
- usage/cache/cost normalization;
- cancellation and malformed stream recovery;
- progress signal and stagnation classification;
- tool-result truncation and artifact retrieval;
- runtime-root isolation;
- generated-file classification;
- failure taxonomy exhaustiveness.
- provenance ledger append and hash-chain validation;
- integration campaign rejection of untrusted or controller-edited patches.

### Integration tests

- fake Responses server through a real `AgentSession`;
- one read/edit/verify loop;
- parallel reads followed by serialized mutation;
- provider error and retry;
- timeout and cancellation cleanup;
- independent verifier and patch replay;
- isolated eval execution while the user session store contains a corrupt
  snapshot.
- independent scoring worktrees from one canonical monorepo baseline;
- sequential trusted-patch integration with full-monorepo verification.

### Paid smokes and experiments

Paid calls occur only after offline conformance passes. Every paid batch starts
with one cheap smoke and stops immediately on provider schema drift, missing
usage, untrusted workspace evidence, or repeated harness contract errors.

## 11. Acceptance and Claim Boundary

Forge may claim "frontier-grade DeepSeek harness" only when:

1. the frozen holdout meets every threshold in Section 1;
2. a clean-checkout reproduction produces the same trust decision;
3. the winning profile and comparison report are committed;
4. no required evidence is mock-only;
5. desktop safety, provider, runtime, and eval gates pass;
6. the ForgeBench integration campaign ends with a clean full-monorepo build;
7. `FORGE-BUILD-MANIFEST.json` verifies the complete Forge provenance chain;
8. all known limitations are explicit.

This project does not claim:

- that DeepSeek V4 Flash is the best model overall;
- that a Forge score is comparable to an external leaderboard without the same
  dataset and harness;
- automatic commit, merge, or push;
- unsupervised approval of dangerous actions;
- true Tauri force-quit/reopen coverage until the platform harness exists;
- billing-grade cost when the provider omits or contradicts usage.

## 12. Documentation and Product Impact

User-visible provider defaults, transport selection, usage facts, or runtime
behavior require synchronized updates to:

- `README.md`;
- `apps/desktop/README.md`;
- `apps/eval-runner/README.md`;
- `CHANGELOG.md`;
- relevant acceptance gates and eval operations docs.

The first implementation plan must start with GitNexus impact analysis for
every symbol it will modify. HIGH or CRITICAL findings require an explicit
warning and a narrower migration sequence before editing.
