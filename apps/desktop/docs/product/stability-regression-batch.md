# Stability Regression Batch

Run these tasks against a disposable current project. Controller-side manual writes invalidate the task.
Record each result with the exact command, diff, screenshot, or final-answer evidence used to judge it.

| # | Task | Expected Permission State | Required Evidence | Result |
| --- | --- | --- | --- | --- |
| 1 | `/fix @src/App.tsx` for a small visible button feedback issue | Trust or Full Access should avoid repeated routine write prompts | final answer, diff, build/check result | Protocol ready 2026-06-27: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`; not yet run end to end. |
| 2 | CSS layout polish in current project | Routine write allowed only inside current workspace | changed files, no external write | Protocol ready 2026-06-27: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`; not yet run end to end. |
| 3 | Build/check command | Safe shell allowed under Full Access | command output summary | Protocol ready 2026-06-27: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`; not yet run end to end. |
| 4 | Preview ownership question | final answer states URL and workspace path | final answer + Project Status details | |
| 5 | `/code-review` | findings-first, calibrated severity | review output | |
| 6 | New conversation same workspace | runtime trust/full access inherited | Composer mode + getPermissionMode args | |
| 7 | External path write attempt | blocked or confirmed | confirm/deny evidence | Automated UI takeover guard passed 2026-06-27: Full Access and Trust did not auto-approve external-path confirmation cards. |
| 8 | Secret-like path write attempt | blocked or confirmed | confirm/deny evidence | Automated UI takeover guard passed 2026-06-27: Trust did not auto-approve `.env` or `.env.local` workspace confirmation cards. |
| 9 | Restart with active task | honest restore or recovery notice | restart smoke evidence | |
| 10 | Context usage after provider event | `余` means true remaining context | Composer label + provider usage row | Automated context remaining evidence passed 2026-06-27: provider usage `411 / 1M` rendered as `余 999.5K`, not the 967K auto-compact threshold. |
