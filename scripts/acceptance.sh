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
  9. Typed completion evidence tests
  10. Gateway loop runner status smoke
  11. Subagent runtime event projection smoke
  12. Live worktree worker lifecycle harness
  13. A2A child runtime file IO bridge
  14. Executor file IO stream smoke
  15. Completion contract desktop helper smoke
  16. Completion contract mocked desktop smoke
  17. Desktop Phase 7 and A2A worker lifecycle smoke specs
  18. Rich preview e2e smoke specs
  19. mocked desktop restart runtime smoke

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
  "typed completion evidence tests"
  "gateway loop runner status smoke"
  "subagent runtime event projection smoke"
  "live worktree worker lifecycle harness"
  "A2A child runtime file IO bridge"
  "executor file IO stream smoke"
  "completion contract desktop helper smoke"
  "completion contract mocked desktop smoke"
  "desktop Phase 7 and A2A worker lifecycle smoke specs"
  "rich preview e2e smoke specs"
  "mocked desktop restart runtime smoke"
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
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib"
  "node --test apps/desktop/src/store/blocks.test.ts"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child::tests::run_worktree_worker --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib"
  "node --test apps/desktop/src/lib/loopRuntime.test.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g \"write_file tool details show|diff cards show|image diff cards show\""
  "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts"
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
