#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0

for arg in "$@"; do
  case "$arg" in
    --dry-run)
      DRY_RUN=1
      ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/acceptance.sh [--dry-run]

Runs the Forge Level 3 runtime acceptance gates:
  1. Desktop production build
  2. Website production build
  3. Eval runner test suite
  4. Loop event journal contract tests
  5. Projection rebuild/replay tests
  6. Policy preflight tests
  7. Budget preflight tests
  8. Durable human gate tests
  9. Gateway loop runner status smoke
  10. Subagent runtime event projection smoke
  11. Live worktree worker lifecycle harness
  12. A2A child runtime file IO bridge
  13. Executor file IO stream smoke
  14. Completion contract desktop helper smoke
  15. Completion contract mocked desktop smoke
  16. Mocked desktop restart runtime smoke (partial macOS evidence)
  17. Desktop restart harness availability preflight
  18. Confirmation response replay contract tests
  19. Desktop UI evidence observer preflight
  20. Desktop UI evidence doctor
  21. Manual desktop restart smoke protocol gate
  22. Manual stability regression batch gate
  23. Manual disposable edit/build loop protocol gate
  24. Disposable edit/build loop project readiness preflight
  25. Disposable edit/build loop clean worktree prepare dry-run
  26. Disposable edit/build loop evidence collector
  27. Disposable edit/build loop evidence validator
  28. Disposable edit/build loop evidence archive dry-run
  29. Disposable edit/build loop manual evidence template
  30. Disposable edit/build loop manual evidence review
  31. Disposable edit/build loop row finalizer dry-run
  32. Disposable edit/build loop row runbook
  33. Disposable edit/build loop status summary
  34. Provider usage known/unknown telemetry
  35. Composer context usage from provider_usage
  36. Provider usage trace rendering
  37. Legacy usage duplicate suppression
  38. Post-shell file-effect evidence smoke (bounded, not shell-internal)
  39. Persisted A2A lineage tests
  40. Typed completion evidence and review-to-commit eligibility tests
  41. Gated headless ownership policy tests
  42. Permission mode, live-session sync, and shell policy contract tests
  43. Slash command review calibration contract tests
  44. Desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs
  45. Rich preview e2e smoke specs

Use --dry-run to print the command plan without executing it.
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

COMMAND_LABELS=(
  "desktop production build"
  "website production build"
  "eval runner test suite"
  "loop event journal contract tests"
  "projection rebuild/replay tests"
  "policy preflight tests"
  "budget preflight tests"
  "durable human gate tests"
  "gateway loop runner status smoke"
  "subagent runtime event projection smoke"
  "live worktree worker lifecycle harness"
  "A2A child runtime file IO bridge"
  "executor file IO stream smoke"
  "completion contract desktop helper smoke"
  "completion contract mocked desktop smoke"
  "mocked desktop restart runtime smoke (partial macOS evidence)"
  "desktop restart harness availability preflight"
  "confirmation response replay contract tests"
  "desktop UI evidence observer preflight"
  "desktop UI evidence doctor"
  "manual desktop restart smoke protocol"
  "manual stability regression batch"
  "manual disposable edit/build loop protocol"
  "disposable edit/build loop project readiness preflight"
  "disposable edit/build loop clean worktree prepare dry-run"
  "disposable edit/build loop evidence collector"
  "disposable edit/build loop evidence validator"
  "disposable edit/build loop evidence archive dry-run"
  "disposable edit/build loop manual evidence template"
  "disposable edit/build loop manual evidence review"
  "disposable edit/build loop row finalizer dry-run"
  "disposable edit/build loop row runbook"
  "disposable edit/build loop status summary"
  "provider usage known/unknown telemetry"
  "composer context usage from provider_usage"
  "provider usage trace rendering"
  "legacy usage duplicate suppression"
  "post-shell file-effect evidence smoke (bounded, not shell-internal)"
  "persisted A2A lineage tests"
  "typed completion evidence and review-to-commit eligibility tests"
  "gated headless ownership policy tests"
  "permission mode, live-session sync, and shell policy contract tests"
  "slash command review calibration contract tests"
  "desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs"
  "rich preview e2e smoke specs"
)

COMMANDS=(
  "npm run build:desktop"
  "npm run build:website"
  "npm run test:eval"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::policy --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib"
  "node --test apps/desktop/src/store/blocks.test.ts"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child::tests::run_worktree_worker --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib"
  "node --test apps/desktop/src/lib/loopRuntime.test.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts"
  "node scripts/desktop-restart-harness-preflight.mjs --json"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g \"confirm response replay|startup transcript hydration\""
  "node scripts/desktop-ui-evidence-preflight.mjs --json"
  "node scripts/desktop-ui-evidence-doctor.mjs --json"
  "test -f apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q \"Stability Convergence Restart Smoke - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "test -f apps/desktop/docs/product/stability-regression-batch.md && rg -q \"Stability Regression Batch - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "test -f apps/desktop/docs/product/phase8-disposable-loop-protocol.md && rg -q \"Phase 8 Disposable Edit/Build Loop - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "node scripts/disposable-loop-preflight.mjs --json"
  "node scripts/prepare-disposable-loop-project.mjs --json --dry-run"
  "node scripts/collect-disposable-loop-evidence.mjs --json"
  "node scripts/validate-disposable-loop-evidence.mjs --json"
  "node scripts/archive-disposable-loop-evidence.mjs --json --dry-run"
  "node scripts/create-disposable-loop-manual-json.mjs --json --row 1"
  "node scripts/review-disposable-loop-manual-json.mjs --json --row 1"
  "node scripts/finalize-disposable-loop-row.mjs --json --dry-run --row 1"
  "node scripts/phase8-disposable-loop-runbook.mjs --json --row 1"
  "node scripts/phase8-disposable-loop-status.mjs --json"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib"
  "npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g \"provider_usage without legacy usage\""
  "npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g \"provider usage\""
  "node --test apps/desktop/src/store/event-dispatch.test.ts"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_handlers --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib"
  "npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g \"write_file tool details show|diff cards show|image diff cards show\""
)

echo "Forge Level 3 runtime acceptance suite"
echo "Working directory: $ROOT_DIR"
echo

for index in "${!COMMANDS[@]}"; do
  label="${COMMAND_LABELS[$index]}"
  command="${COMMANDS[$index]}"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $label: $command"
  else
    echo "==> $label"
    echo "    $command"
    (cd "$ROOT_DIR" && eval "$command")
  fi
done
