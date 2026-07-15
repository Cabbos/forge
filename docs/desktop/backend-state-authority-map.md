# Forge Backend State Authority Map

Status: Verified on 2026-07-01. Mapped domains: 15.

This inventory is documentation-only. It records current backend ownership before any consolidation work, so later phases can move state deliberately instead of rewriting behavior by accident.

## Authority Map

| Domain | Current Owner | Durable Store | Projection Consumer | Known Drift |
|---|---|---|---|---|
| Live sessions and restart | `AppState.sessions`, `AgentSession`, `agent::snapshot`, transcript replay | session snapshot JSON + session transcript JSONL | SessionView, History, startup restore, gateway attach, desktop restart harness | live status, active tool calls, latest workflow, and replayed transcript can disagree after interruption or compaction |
| Pending confirmations | `Harness.pending_confirms`, `Harness.pending_confirm_descriptors`, `ipc::confirmations`, transcript events | snapshot `pending_confirms` descriptors + transcript `confirm_ask` / `confirm_response` | ConfirmCard, ProjectStatusCard, startup transcript hydration, eval trace summaries | auto-decision and interrupted-restored reasons are not yet one ledgered permission record |
| Memory recall and project records | unified memory IPC plus source stores: wiki memory, memory facts, continuity, Forge Wiki | wiki JSON, memory fact JSON, continuity DB, Forge Wiki store, transcript selection events | Project Archive, send-input hidden context, memory selection blocks | read path is unified, but archive/forget/edit actions and source retention are still source-specific |
| Context usage and turn preparation | `send_input_context`, `ContextBuilder`, `PreparedSendInputTurnContext`, `AgentTurnState` context snapshot | turn state in session snapshot + transcript selection/status events | Composer context display, AgentTurn projection, usage traces, eval traces | pre-run estimate, hidden context body, selected memory ids, and post-run provider usage are not one contract |
| Workflow, slash command, and capability routing | `setup_send_input_workflow`, `workflow_states`, `AgentTurnMetadata`, capability context builder | snapshot `latest_workflow` + transcript `workflow_updated` + turn metadata | Composer, slash command review calibration, CapabilityDrawer, SessionView | workflow classification is replayable, but capability snapshot inputs are rebuilt instead of fully ledgered |
| Provider usage and budget accounting | `StreamEvent::ProviderUsage`, adapter usage emitters, `AgentTurnState`, `loop_runtime::budget` | transcript `provider_usage` events + session turn state + loop usage ledger / budget snapshot | Composer, Messages usage trace, persistence hydration, loop runtime budget tests | legacy `usage` still exists and must be suppressed or reconciled against canonical `provider_usage` |
| Loop tasks and completion evidence | `loop_runtime` event journal, projection, runner, completion evaluator | `~/.forge/loop-events.jsonl` + `~/.forge/loop-tasks.json` | project runtime/status UI, gateway runtime status, completion helpers, review-to-commit checks | not yet the only task truth for local sessions, A2A work, gateway runs, manual evidence, and eval traces |
| Gateway trigger runs, session input, and dashboard | gateway runner/store, trigger store, session input store, gateway protocol/server | `~/.forge/trigger-runs.json`, gateway trigger queue, session input JSON, session registry | gateway dashboard/status, `forge_trigger`, diagnostics IPC, gateway attach/tail events | gateway run records and local loop tasks are related but not equivalent execution authorities |
| Eval traces and headless ownership | `eval_headless::trace`, eval runner reports, loop runtime headless owner records | trace artifacts + loop runtime `HeadlessOwnerRun*` events | eval runner reports, gated headless ownership policy, gateway trigger runner | traces are strong for eval/headless runs but are not promoted from every desktop turn |
| A2A and subagent runtime lineage | `agent::a2a` bus/ledger/projection, subagent runtime events, A2A child worker | session snapshot A2A state + transcript `agent_a2a_updated` / `subagent_runtime_event` + loop subagent file IO events | HubPanelHost, StatusBar background tasks, A2A review queue/history, subagent runtime blocks | parent/child task ids, worktree worker state, and loop task ids can still live in separate projections |
| Permission, trust mode, and shell policy | `PermissionGate`, permission IPC handlers, harness permission DB, shell policy classifier | harness permission DB + runtime session/workspace permission state + transcript confirmations | Settings permissions, Composer trust mode, ConfirmCard, trust-loop smoke specs | trust/full-access mode, shell policy decisions, and human gates are not yet one policy ledger |
| Scheduler and background triggers | `SchedulerStore`, scheduler tick/run-now paths, gateway trigger queue | `~/.forge/scheduler.json` + gateway trigger queue/run records | Settings scheduler, StatusBar task rows, gateway runner/status | scheduler owns declarative tasks and history, while gateway owns execution attempts |
| File IO, shell effects, and rich previews | executor file IO stream, A2A child file IO bridge, shell file-effect detector, diff/image preview events | transcript `file_io` / `diff_view` / tool events + loop `SubagentFileIoRecorded` + manual evidence JSON | Messages rich preview cards, evidence collector/validator, delivery summary, eval traces | file IO events, post-shell effect detection, and rich preview rendering are related but not one evidence model |
| Delivery, preview ownership, and review status | `delivery_states`, `DeliverySummary`, `AgentTurnState`, loop completion/review records | session snapshot `latest_delivery` + transcript `delivery_summary` + loop `EvidenceRecord` / completion result | ProjectStatusCard, StatusBar, Workbench previews, review-to-commit eligibility | preview ownership and delivery evidence can be computed from session turn state or loop evidence depending on path |
| Diagnostics, health, restart, and UI evidence | diagnostics watchdog/reporting, `StreamEvent::DiagnosticsUpdate`, `HealthAlert`, restart and UI evidence scripts | diagnostics report JSON, transcript health events, product evidence docs/JSON | Settings diagnostics, StatusBar health alert, desktop restart harness, UI evidence doctor | manual evidence, preflight status, and runtime health alerts are not yet generated from one durable fact stream |

## Source Evidence

- `apps/desktop/src-tauri/src/state.rs` defines `AppState.sessions`, `pending_confirms`, memory/profile/continuity stores, workflow/delivery in-memory projections, and `SchedulerStore`.
- `apps/desktop/src-tauri/src/protocol/events.rs` defines the backend to frontend stream contract for confirmations, memory selections, workflow/turn/A2A projections, loop runtime updates, delivery summaries, provider usage, diagnostics, and health alerts.
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs` builds workflow, memory, project record, connector, capability, and hidden-context state for send-input turns.
- `apps/desktop/src-tauri/src/loop_runtime/types.rs`, `journal.rs`, `projection.rs`, and `store.rs` define the durable loop event journal, task projection, policies, budgets, human gates, headless ownership, usage ledgers, and completion evidence.
- `apps/desktop/src-tauri/src/gateway/protocol.rs`, `gateway/runner.rs`, and `gateway/session_input.rs` define gateway trigger runs, loop task APIs, session inbox records, dashboard/status contracts, and session attach/tail contracts.
- `apps/desktop/src-tauri/src/eval_headless/trace.rs` builds the eval/headless trace payload from raw stream events, latest turn state, file diffs, changed files, verification, usage, confirmations, and failure fields.
- `apps/desktop/src-tauri/src/transcript.rs` persists emitted stream events to session transcript JSONL and also feeds autosave/watchdog hooks.
- `apps/desktop/src-tauri/src/agent/snapshot.rs` persists session snapshots, latest turn/workflow/delivery, A2A state, pending confirms, and interrupted tool call descriptors.

## GitNexus Verification

Required command:

```bash
python3 -c 'import subprocess, sys; subprocess.run(["node", ".gitnexus/run.cjs", "query", "session runtime memory context gateway eval trace", "-r", "forge", "-l", "10"], check=True, timeout=60)'
```

Result on 2026-07-01: succeeded and returned related processes, including gateway runtime status/dashboard flows and `send_input`. GitNexus MCP context also reported the index is 7 commits behind HEAD, so the map was checked against direct source reads.

## Acceptance Coverage

Every backend-facing surface mentioned in `scripts/acceptance.sh` is covered by a domain above. Build-only gates remain listed here as acceptance harness coverage, not backend state owners.

| Acceptance gate group | Covered state domain |
|---|---|
| acceptance matrix contract tests | Acceptance harness contract; no backend state owner |
| desktop production build | Cross-app build guard; no backend state owner |
| website production build | Cross-app build guard; no backend state owner |
| eval runner test suite | Eval traces and headless ownership |
| loop event journal, replay, policy, budget, durable human gate tests | Loop tasks and completion evidence; Permission, trust mode, and shell policy |
| gateway loop runner status smoke | Gateway trigger runs, session input, and dashboard; Loop tasks and completion evidence |
| subagent runtime event projection smoke | A2A and subagent runtime lineage |
| live worktree worker lifecycle harness, A2A child runtime file IO bridge | A2A and subagent runtime lineage; File IO, shell effects, and rich previews |
| executor file IO stream smoke | File IO, shell effects, and rich previews |
| completion contract desktop helper and mocked desktop smoke | Loop tasks and completion evidence; Delivery, preview ownership, and review status |
| mocked desktop restart runtime smoke and restart harness preflights/docs | Live sessions and restart; Diagnostics, health, restart, and UI evidence |
| confirmation response replay contract tests | Pending confirmations; Live sessions and restart |
| desktop UI evidence observer/doctor/recovery checks | Diagnostics, health, restart, and UI evidence |
| manual desktop restart smoke, stability regression batch, disposable edit/build loop protocols and evidence tooling | Diagnostics, health, restart, and UI evidence; File IO, shell effects, and rich previews; Delivery, preview ownership, and review status |
| provider usage telemetry, composer context usage, trace rendering, duplicate suppression, transcript usage hydration, state consistency map status | Provider usage and budget accounting; Context usage and turn preparation |
| post-shell file-effect evidence smoke | File IO, shell effects, and rich previews |
| persisted A2A lineage tests | A2A and subagent runtime lineage |
| typed completion evidence and review-to-commit eligibility tests | Loop tasks and completion evidence; Delivery, preview ownership, and review status |
| gated headless ownership policy tests | Eval traces and headless ownership; Loop tasks and completion evidence |
| permission mode, live-session sync, and shell policy contract tests | Permission, trust mode, and shell policy; Live sessions and restart |
| slash command review calibration contract tests | Workflow, slash command, and capability routing |
| desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs | Permission, trust mode, and shell policy; Delivery, preview ownership, and review status; Diagnostics, health, restart, and UI evidence; Pending confirmations; Workflow, slash command, and capability routing |
| rich preview e2e smoke specs | File IO, shell effects, and rich previews |

## Phase 0 Guardrail

No production code should change in Phase 0. Consolidation can start only after this map exists, the roadmap records the mapped domain count, and documentation-only diffs pass whitespace checks.
