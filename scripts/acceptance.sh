#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0
LIST_JSON=0
SHOW_HELP=0
ONLY_LABEL=""

while [[ "$#" -gt 0 ]]; do
  arg="$1"
  case "$arg" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --list-json)
      LIST_JSON=1
      shift
      ;;
    --only)
      if [[ "$#" -lt 2 ]]; then
        echo "Missing value for --only" >&2
        exit 2
      fi
      ONLY_LABEL="$2"
      shift 2
      ;;
    -h|--help)
      SHOW_HELP=1
      shift
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
  "desktop restart harness preflight contract tests"
  "desktop restart harness blocker documentation status"
  "confirmation response replay contract tests"
  "desktop UI evidence observer preflight"
  "desktop UI evidence doctor"
  "desktop UI evidence recovery checks"
  "manual desktop restart smoke protocol"
  "manual stability regression batch"
  "manual disposable edit/build loop protocol"
  "disposable edit/build loop beta-log archive status"
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
  "disposable edit/build loop live-ready hard gate"
  "provider usage known/unknown telemetry"
  "composer context usage from provider_usage"
  "provider usage trace rendering"
  "legacy usage duplicate suppression"
  "legacy transcript usage hydration"
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
  "node --test scripts/desktop-restart-harness-preflight.test.mjs"
  "rg -q \"official macOS WKWebView WebDriver support\" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q \"node --test scripts/desktop-restart-harness-preflight.test.mjs\" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q \"blocked_official_macos\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q \"official macOS WKWebView WebDriver support\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q \"desktop-restart-harness-preflight.test.mjs\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g \"confirm response replay|startup transcript hydration\""
  "node scripts/desktop-ui-evidence-preflight.mjs --json"
  "node scripts/desktop-ui-evidence-doctor.mjs --json"
  "node scripts/desktop-ui-evidence-doctor.mjs --json --run-checks"
  "test -f apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q \"Stability Convergence Restart Smoke - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "test -f apps/desktop/docs/product/stability-regression-batch.md && rg -q \"Stability Regression Batch - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "test -f apps/desktop/docs/product/phase8-disposable-loop-protocol.md && rg -q \"Phase 8 Disposable Edit/Build Loop - 2026-06-27\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
  "test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-1.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-2.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-3.validation.json && rg -q \"Status: Archived complete for rows #1/#2/#3\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q \"2026-06-28-row-1\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q \"2026-06-28-row-2\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q \"2026-06-28-row-3\" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md"
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
  "node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib"
  "npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g \"provider_usage without legacy usage\""
  "npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g \"provider usage\""
  "node --test apps/desktop/src/store/event-dispatch.test.ts"
  "node --test apps/desktop/src/store/persistence-hydration.test.ts"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_handlers --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib"
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib"
  "npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts"
  "npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g \"write_file tool details show|diff cards show|image diff cards show\""
)

if [[ "${#COMMAND_LABELS[@]}" -ne "${#COMMANDS[@]}" ]]; then
  echo "Acceptance matrix mismatch: ${#COMMAND_LABELS[@]} labels for ${#COMMANDS[@]} commands" >&2
  exit 1
fi

SELECTED_INDICES=()
for index in "${!COMMANDS[@]}"; do
  if [[ -z "$ONLY_LABEL" || "${COMMAND_LABELS[$index]}" == "$ONLY_LABEL" ]]; then
    SELECTED_INDICES+=("$index")
  fi
done

if [[ -n "$ONLY_LABEL" && "${#SELECTED_INDICES[@]}" -eq 0 ]]; then
  echo "No acceptance gate matched --only: $ONLY_LABEL" >&2
  read -r -a only_words <<< "$ONLY_LABEL"
  for label in "${COMMAND_LABELS[@]}"; do
    suggestion_match=1
    for word in "${only_words[@]}"; do
      if [[ "$label" != *"$word"* ]]; then
        suggestion_match=0
        break
      fi
    done
    if [[ "$suggestion_match" -eq 1 ]]; then
      echo "Did you mean: $label" >&2
      break
    fi
  done
  echo "Run scripts/acceptance.sh --list-json to see valid labels." >&2
  exit 2
fi

if [[ "$SHOW_HELP" -eq 1 ]]; then
  cat <<'EOF'
Usage: scripts/acceptance.sh [--dry-run] [--list-json] [--only <label>]

Runs the Forge Level 3 runtime acceptance gates:
EOF
  for index in "${!COMMAND_LABELS[@]}"; do
    printf '  %s. %s\n' "$((index + 1))" "${COMMAND_LABELS[$index]}"
  done
  cat <<'EOF'

Use --dry-run to print the command plan without executing it.
Use --list-json to print the same gate plan as machine-readable JSON.
Use --only with an exact gate label to run or dry-run one gate.
EOF
  exit 0
fi

if [[ "$LIST_JSON" -eq 1 ]]; then
  {
    printf '%s\0' "$ROOT_DIR"
    for index in "${SELECTED_INDICES[@]}"; do
      printf '%s\0%s\0%s\0' "$((index + 1))" "${COMMAND_LABELS[$index]}" "${COMMANDS[$index]}"
    done
  } | node -e '
const fs = require("node:fs");
const parts = fs.readFileSync(0).toString("utf8").split("\0");
const workingDirectory = parts.shift();
if (parts.at(-1) === "") parts.pop();
const gates = [];
for (let index = 0; index < parts.length; index += 3) {
  gates.push({
    index: Number(parts[index]),
    label: parts[index + 1],
    command: parts[index + 2],
  });
}
process.stdout.write(JSON.stringify({ schemaVersion: 1, workingDirectory, gates }, null, 2) + "\n");
'
  exit 0
fi

echo "Forge Level 3 runtime acceptance suite"
echo "Working directory: $ROOT_DIR"
echo

for index in "${SELECTED_INDICES[@]}"; do
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
