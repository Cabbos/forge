# Desktop State Consistency Map

> Last updated: 2026-06-28
> Scope: Forge desktop internal beta stability convergence.

## Contract

Every visible state in Forge must name one source of truth, all replicas, the sync trigger, the failure mode if sync is missed, and the proof command or test that catches drift.

## State Surfaces

| State | Source of Truth | Replicas | Sync Trigger | Known Drift Failure | Proof |
| --- | --- | --- | --- | --- | --- |
| Active workspace | Rust `Session` workspace plus frontend working-dir mirror | `localStorage["forge-working-dir"]`, Project Status card, Composer permission control | session creation, workspace selection, hydration | UI says project A while live harness checks project B | `apps/desktop/e2e/acceptance.spec.ts` permission/workspace specs |
| Permission mode | Rust `PermissionGate` app-level workspace mode and live session harness gate | permission IPC payload, Composer control, Settings > Tools, Project Status card | mode read/mutation, session creation, before `send_input` | UI says trusted/full access but live session still asks | `cargo test ... harness::permissions`, `cargo test ... ipc::permission_handlers`, and `acceptance.spec.ts -g "permission|trust|full access"` |
| Pending confirmation | Rust `AppState.pending_confirms` plus frontend `confirm_ask` block | Confirm card, Project Status action, Composer action | `confirm_ask`, `confirm_response` IPC, replayed/interrupted metadata, mode takeover | enabling trust approves wrong confirmation or visible card stays pending | `acceptance.spec.ts` tests containing `pending` / `confirmation` plus store replay confirmation tests |
| Session status | Rust `SessionStatus` events | Zustand `SessionState.status`, sidebars, health banners | `session_status`, restore replay, stop/kill | stale running/resuming state after idle/completed | Rust watchdog/session tests plus `apps/desktop/src/store/health-alerts.test.ts` |
| Usage/context | canonical `provider_usage` event projected into `usageLedger`; legacy `usage` is replay fallback only | provider usage block, session cost, Composer context label, IndexedDB, Tauri transcript replay | provider event, legacy usage fallback, `transcript usage hydration`, context compacted | Composer `余` disagrees with provider facts after reload or restart | `usage-ledger.test.mjs`, `event-dispatch.test.ts`, `persistence-hydration.test.ts`, `contextUsageView.test.mjs`, `scripts/acceptance.sh --only "transcript usage hydration"` |
| Health alerts | Rust `HealthAlert` events plus frontend active-session filter | `healthAlerts` store, `HealthAlertBanner`, StatusBar | health alert event, fresh same-session stream event | stale banner remains after fresh output or appears for another active session | `health-alerts.test.ts` and `acceptance.spec.ts -g "health alert|stale alert"` |
| Preview ownership | Rust turn evidence and project runtime status | final answer instruction, Project Status details, delivery summary | preview probe/status event, finalization | URL shown without workspace ownership | `apps/desktop/e2e/acceptance.spec.ts -g "preview ownership"` plus Rust turn-outcome coverage |

## Current Required Gates

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/components/session/contextUsageView.test.mjs apps/desktop/src/store/health-alerts.test.ts apps/desktop/src/lib/ipc/permissions.test.ts scripts/acceptance.test.mjs
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|trust|full access|health alert|stale alert|preview ownership"
npm run build:desktop
npm --prefix apps/desktop run check:backend
scripts/acceptance.sh --only "transcript usage hydration"
scripts/acceptance.sh --only "desktop state consistency map status"
scripts/acceptance.sh --dry-run
git diff --check
```
