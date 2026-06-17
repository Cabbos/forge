# Changelog

## Unreleased

- Expanded Forge Eval Runner into a trusted backtest workflow with trust gates, dataset fingerprints, experiment snapshots, layered scorers, red-team lanes, sandbox leakage checks, report comparison, trace promotion, queue status, worker cancellation diagnostics, trajectory artifacts, cost budgets, and PASS_TO_PASS / FAIL_TO_PASS validation.
- Added desktop session History management coverage: search, provider filtering, resume, delete, rename, export, and conservative prune flows.
- Expanded Settings runtime surfaces for diagnostics, gateway runtime status, service/autostart visibility, scheduler CRUD/run/disable/delete, and permission allow/deny/reset controls.
- Added A2A review summaries in Agent Workbench, including review queue details, changed-file chips, suggested actions, and review rejection history.
- Added a global background status bar with expandable task rows for agent work, review items, scheduler tasks, and health alerts.
- Added richer tool previews for Markdown write/edit operations and compact multi-file diff tree summaries.
- Added `scripts/acceptance.sh` and browser acceptance coverage for resume, diagnostics, permissions, scheduler, A2A review, and background task UI.
