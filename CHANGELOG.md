# Changelog

## Unreleased

- Added executor-level `file_io` stream events for successful direct ToolExecutor file-ish calls (`read`, `write`, `edit`, `diff`, `list`, and `search`) and conservative frontend metadata projection onto existing tool/shell transcript blocks. This does not claim shell-internal file effect tracing, provider token/cost streaming, gateway autonomous resume, automatic parent selection, parent-session structs, or auto commit/merge/push.
- Added a narrow A2A child runtime live event bridge: delegated read-only, patch-proposal, and worktree-worker children can now emit `subagent_runtime_event` lifecycle and successful file-ish tool facts with the parent session id and A2A task id. This is not shell-internal file effect tracing, gateway autonomous resume, automatic parent selection, auto commit/merge/push, or provider token/cost streaming.
- Added Level 3 loop runtime acceptance coverage for the append-only event journal, rebuildable projections, durable human gates, policy/budget preflight, typed completion evidence, crash/replay regressions, gateway runner leases, subagent runtime projection, mocked desktop completion-contract smoke, direct executor file-IO stream smoke, and the real Rust `run_worktree_worker` harness using the mock adapter/harness. This does not claim a Tauri/WebDriver force-quit harness, shell-internal file effect tracing, or a provider cost stream.
- Expanded Forge Eval Runner into a trusted backtest workflow with trust gates, dataset fingerprints, experiment snapshots, layered scorers, red-team lanes, sandbox leakage checks, report comparison, trace promotion, queue status, worker cancellation diagnostics, trajectory artifacts, cost budgets, and PASS_TO_PASS / FAIL_TO_PASS validation.
- Added desktop session History management coverage: search, provider filtering, resume, delete, rename, export, and conservative prune flows.
- Expanded Settings runtime surfaces for diagnostics, gateway runtime status, service/autostart visibility, scheduler CRUD/run/disable/delete, and permission allow/deny/reset controls.
- Added A2A review summaries in Agent Workbench, including review queue details, changed-file chips, suggested actions, and review rejection history.
- Added projection-only A2A parent lineage visibility in Agent Workbench/Timeline, showing derived child counts without persisting parent-side child arrays or changing automatic parent selection.
- Added a global background status bar with expandable task rows for agent work, review items, scheduler tasks, and health alerts.
- Added richer tool previews for Markdown write/edit operations and compact multi-file diff tree summaries.
- Added `scripts/acceptance.sh` and browser acceptance coverage for resume, diagnostics, permissions, scheduler, A2A review, and background task UI.
