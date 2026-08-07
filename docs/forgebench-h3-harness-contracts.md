# ForgeBench H3 Harness Contract Evidence

H3 is the first offline convergence slice after the DeepSeek H2 transport
comparison. It does not claim a paid score improvement. It makes the evidence
needed for frontier tuning explicit and bounded before another paid campaign.

## Contracts

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

Every Forge trace now emits one `harness_contract_evidence` raw event. Legacy
shell tool calls are converted into bounded envelopes, while agent-provided
envelopes/signals are validated strictly. Missing progress signals are marked
`source: missing`; they are never inferred as success or silently treated as
progress.

Baseline reports aggregate the event into `harness_telemetry`: contract
coverage, tool-call histogram and repeated-call rate, context byte/token
percentiles and tier totals, duplicate/conflict rates, stagnation redirects and
stops, and single-repair compliance. Missing contract events remain visible as
reduced coverage instead of being silently backfilled.

Paired comparison artifacts also publish deterministic 2,000-resample bootstrap
intervals for success, verification, scope, rounds, and duration deltas. The
seed and method are serialized with the artifact so a later report cannot
quietly change the uncertainty calculation.

## Verification

Run the offline gate:

```bash
scripts/acceptance.sh --only "frontier harness contract evidence"
```

The gate covers the pure contract tests and a Forge runner integration test.
This is development-split harness evidence only. H4 holdout execution remains
closed until a complete, trusted development A/B comparison exists and all
paid interruptions have a known charge or an explicit unknown-cost blocker.
