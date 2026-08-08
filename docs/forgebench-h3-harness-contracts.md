# ForgeBench H3 Harness Contract Evidence

H3 is the first offline convergence slice after the DeepSeek H2 transport
comparison. It does not claim a paid score improvement. It makes the evidence
needed for frontier tuning explicit and bounded before another paid campaign.

## Contracts

- `HarnessPolicyV1` gives measured eval runs an explicit `minimal_v1` prompt and
  `task_local_v1` context mode. It caps the selected context list, renders a
  compact inspect/edit/verify/one-repair contract, and fingerprints the policy
  separately from provider credentials.
- The committed minimal profiles are
  `deepseek-v4-flash-anthropic-minimal-v1` and
  `deepseek-v4-flash-openai-chat-minimal-v1`. They are route-bound canonical
  artifacts; H1 keeps its legacy omitted `base_url`, while H2 records the
  explicit OpenAI-compatible URL.
- `ContextManifest` records only selected context paths, tier, origin, priority,
  byte/token estimates, content hashes, duplicate groups, and same-name
  conflicts. Source content is never copied into the manifest.
- `ToolResultEnvelope` caps summary, facts, content, and next-evidence fields.
  Oversized output is marked `truncated`, carries a SHA-256 digest, and points
  to retrieval evidence instead of silently inflating the prompt/trace.
- `ProgressSignal` and `StagnationDecision` make no-progress rounds typed. Two
  consecutive no-progress rounds allow one bounded recovery redirect; a second
  pair stops with an explicit reason.
- `FailureTaxonomyEvidence` maps legacy runner outcomes to one explicit primary
  category (`passed`, model, provider, tool, permission, verification, scope,
  observer, replay, contract, cancellation, or timeout) so reports never need
  to expose a generic `runner_error` as the terminal cause.
- Forge eval prompts now render the controller-owned allowed and forbidden file
  scope, plus a bounded “targeted validation then stop” convergence instruction,
  before the first edit. Post-run filesystem snapshots remain authoritative;
  this prompt is an early guard against the scope and round failures seen in the
  first paid H4 control result, not a substitute for the scope gate.
- Forge eval prompts now also receive a controller-owned `TaskExecutionContract`.
  It classifies no-op, single-file, multi-file, and shell-heavy tasks, names the
  files that must be inspected, carries only the targeted validation commands,
  and states the stop condition. A headless adapter may return a bounded
  `task_plan`; when present, the runner validates unique phases, required
  inspection coverage, in-scope edits, and a verification step. Missing plans
  remain explicit evidence rather than being counted as success.

Every Forge trace now emits one `harness_contract_evidence` raw event. Legacy
shell tool calls are converted into bounded envelopes, while agent-provided
envelopes/signals are validated strictly. Missing progress signals are marked
`source: missing`; they are never inferred as success or silently treated as
progress. The event also reports advisory observed phase coverage (which
required paths appeared in inspection commands, which files actually changed,
and whether controller verification passed); this is never substituted for the
filesystem snapshot or patch-replay authority.

The first post-H4 development probe on the scope prompt guard exposed one
additional observer false positive: an OpenAI arm created `.uv-cache` inside
the fixture even though `UV_CACHE_DIR` was isolated. The control arm still
closed 30/30 with 0.9000 trusted success, 0.0333 scope violations, and 26.5
median rounds; the candidate stopped safely at 23/30 when the cache files made
patch replay untrusted. `.uv-cache` is now an explicit disposable-cache
allowlist entry alongside `.bun-cache`; the interrupted candidate is not
replayed or promoted to a comparison result.

After that fix, a fresh source-pinned 30-attempt arm pair completed with no
trust blockers. Control was 0.9333 trusted success, 0.0667 scope violations,
and 23.0 median rounds; candidate was 0.9333, 0.0333, and 30.0 respectively.
The paired bootstrap comparison digest is
`26e2d36cd5f9c42dd2e6ae8514154e3c6bd3e140b827cb31fb828dfcc2214d65`, and its
quality gate selected `no_material_difference`: the scope interval crossed
zero and the round delta favored control. This is useful harness evidence, not
a claim that the prompt guard alone improved model quality.

Both H4 holdout and H2 development campaign verifiers now expose this same
publication boundary: the default path remains strict for active execution,
while `--frozen` rechecks sealed artifacts after documentation commits without
requiring the live worktree digest and without starting a provider call.

Baseline reports aggregate the event into `harness_telemetry`: contract
coverage, tool-call histogram and repeated-call rate, context byte/token
percentiles and tier totals, duplicate/conflict rates, stagnation redirects and
stops, fresh-controller-verifier rate, and single-repair compliance. The
controller rejects a trace that reports more repairs than the configured limit.
Missing contract events remain visible as reduced coverage instead of being
silently backfilled.

Paired comparison artifacts also publish deterministic 2,000-resample bootstrap
intervals for success, verification, scope, rounds, and duration deltas. The
seed and method are serialized with the artifact so a later report cannot
quietly change the uncertainty calculation. Winner selection is
`paired_bootstrap_quality_gated_v1`: a candidate needs a confidence-separated
success gain or efficiency gain, with verification and scope non-inferiority;
ambiguous intervals remain `no_material_difference` instead of being promoted
to a winner.

## Verification

Run the offline gate:

```bash
scripts/acceptance.sh --only "frontier harness contract evidence"
```

The gate covers the canonical minimal profiles, pure contract tests, and a Forge
runner integration test.
This is development-split harness evidence only. H4 holdout execution remains
closed until a complete, trusted development A/B comparison exists and all
paid interruptions have a known charge or an explicit unknown-cost blocker.
