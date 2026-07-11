#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0
LIST_JSON=0
SHOW_HELP=0
ONLY_LABEL=""
GREP_LABEL=""
GREP_LABEL_MATCH=""
CI_DEFAULT=0
RESULTS_JSON=""
RELEASE_PROFILE=""
REQUIRE_STATE=""

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
      if [[ "$#" -lt 2 || -z "$2" ]]; then
        echo "Missing value for --only" >&2
        exit 2
      fi
      ONLY_LABEL="$2"
      shift 2
      ;;
    --grep)
      if [[ "$#" -lt 2 || -z "$2" ]]; then
        echo "Missing value for --grep" >&2
        exit 2
      fi
      GREP_LABEL="$2"
      shift 2
      ;;
    --ci-default)
      CI_DEFAULT=1
      shift
      ;;
    --release-profile)
      if [[ "$#" -lt 2 || -z "$2" ]]; then
        echo "Missing value for --release-profile" >&2
        exit 2
      fi
      RELEASE_PROFILE="$2"
      shift 2
      ;;
    --require-state)
      if [[ "$#" -lt 2 || -z "$2" ]]; then
        echo "Missing value for --require-state" >&2
        exit 2
      fi
      REQUIRE_STATE="$2"
      shift 2
      ;;
    --results-json)
      if [[ "$#" -lt 2 || -z "$2" ]]; then
        echo "Missing value for --results-json" >&2
        exit 2
      fi
      RESULTS_JSON="$2"
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

selector_count=0
[[ -n "$ONLY_LABEL" ]] && selector_count=$((selector_count + 1))
[[ -n "$GREP_LABEL" ]] && selector_count=$((selector_count + 1))
[[ "$CI_DEFAULT" -eq 1 ]] && selector_count=$((selector_count + 1))
[[ -n "$RELEASE_PROFILE" ]] && selector_count=$((selector_count + 1))
if [[ "$selector_count" -gt 1 ]]; then
  echo "Use only one selector: --only, --grep, --ci-default, or --release-profile." >&2
  exit 2
fi

if [[ -n "$RELEASE_PROFILE" && -z "$REQUIRE_STATE" ]]; then
  echo "Use --require-state with --release-profile." >&2
  exit 2
fi
if [[ -z "$RELEASE_PROFILE" && -n "$REQUIRE_STATE" ]]; then
  echo "Use --require-state only with --release-profile." >&2
  exit 2
fi

if [[ -n "$RESULTS_JSON" && ( "$DRY_RUN" -eq 1 || "$LIST_JSON" -eq 1 || "$SHOW_HELP" -eq 1 ) ]]; then
  echo "Use --results-json only when executing gates." >&2
  exit 2
fi

if [[ -n "$GREP_LABEL" ]]; then
  GREP_LABEL_MATCH="$(printf '%s' "$GREP_LABEL" | tr '[:upper:]' '[:lower:]')"
fi

COMMAND_LABELS=()
COMMANDS=()
GATE_DOMAINS=()
RESULT_LABELS=()
RESULT_COMMANDS=()
RESULT_DOMAINS=()
RESULT_TIERS=()
RESULT_RUNTIME_COSTS=()
RESULT_MANUAL_REQUIREMENTS=()
RESULT_CI_DEFAULTS=()
RESULT_STATUSES=()
RESULT_EXIT_CODES=()
RESULT_DURATION_MS=()
RESULT_REASONS=()
RESULT_EXECUTION_STATUSES=()
RESULT_CONDITION_STATUSES=()
RESULT_STARTED_AT_MS=()
RESULT_FINISHED_AT_MS=()
CURRENT_DOMAIN="foundation"

set_domain() {
  CURRENT_DOMAIN="$1"
}

# Keep each gate as one add_gate call so label/command order cannot drift.
add_gate() {
  GATE_DOMAINS+=("$CURRENT_DOMAIN")
  COMMAND_LABELS+=("$1")
  COMMANDS+=("$2")
}

now_ms() {
  node -e 'process.stdout.write(String(Date.now()))'
}

record_gate_result() {
  local label="$1"
  local command="$2"
  local domain="$3"
  local tier="$4"
  local runtime_cost="$5"
  local manual_requirement="$6"
  local ci_default="$7"
  local exit_code="$8"
  local duration_ms="$9"
  local started_at_ms="${10}"
  local finished_at_ms="${11}"
  local status="passed"
  local reason=""
  local execution_status="completed"
  local condition_status="passed"

  if [[ "$exit_code" -eq 126 || "$exit_code" -eq 127 || "$exit_code" -eq 130 || "$exit_code" -eq 137 || "$exit_code" -eq 143 ]]; then
    status="unknown"
    execution_status="execution_failed"
    condition_status="unknown"
    reason="execution_exit_code_$exit_code"
  elif [[ "$exit_code" -ne 0 ]]; then
    status="failed"
    condition_status="failed"
    reason="exit_code_$exit_code"
  fi

  RESULT_LABELS+=("$label")
  RESULT_COMMANDS+=("$command")
  RESULT_DOMAINS+=("$domain")
  RESULT_TIERS+=("$tier")
  RESULT_RUNTIME_COSTS+=("$runtime_cost")
  RESULT_MANUAL_REQUIREMENTS+=("$manual_requirement")
  RESULT_CI_DEFAULTS+=("$ci_default")
  RESULT_STATUSES+=("$status")
  RESULT_EXIT_CODES+=("$exit_code")
  RESULT_DURATION_MS+=("$duration_ms")
  RESULT_REASONS+=("$reason")
  RESULT_EXECUTION_STATUSES+=("$execution_status")
  RESULT_CONDITION_STATUSES+=("$condition_status")
  RESULT_STARTED_AT_MS+=("$started_at_ms")
  RESULT_FINISHED_AT_MS+=("$finished_at_ms")
}

write_results_json() {
  local overall_exit_code="$1"
  if [[ -z "$RESULTS_JSON" ]]; then
    return
  fi

  {
    printf '%s\0%s\0%s\0' "$ROOT_DIR" "$overall_exit_code" "${#SELECTED_INDICES[@]}"
    for index in "${!RESULT_LABELS[@]}"; do
      printf '%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0%s\0' \
        "${RESULT_LABELS[$index]}" \
        "${RESULT_COMMANDS[$index]}" \
        "${RESULT_DOMAINS[$index]}" \
        "${RESULT_TIERS[$index]}" \
        "${RESULT_RUNTIME_COSTS[$index]}" \
        "${RESULT_MANUAL_REQUIREMENTS[$index]}" \
        "${RESULT_CI_DEFAULTS[$index]}" \
        "${RESULT_STATUSES[$index]}" \
        "${RESULT_EXIT_CODES[$index]}" \
        "${RESULT_DURATION_MS[$index]}" \
        "${RESULT_REASONS[$index]}" \
        "${RESULT_EXECUTION_STATUSES[$index]}" \
        "${RESULT_CONDITION_STATUSES[$index]}" \
        "${RESULT_STARTED_AT_MS[$index]}" \
        "${RESULT_FINISHED_AT_MS[$index]}"
    done
  } | node -e '
const fs = require("node:fs");
const outputPath = process.argv[1];
const parts = fs.readFileSync(0).toString("utf8").split("\0");
const workingDirectory = parts.shift();
const overallExitCode = Number(parts.shift());
const selectedGateCount = Number(parts.shift());
if (parts.at(-1) === "") parts.pop();

const gates = [];
for (let index = 0; index < parts.length; index += 15) {
  gates.push({
    label: parts[index],
    command: parts[index + 1],
    domain: parts[index + 2],
    tier: parts[index + 3],
    runtimeCost: parts[index + 4],
    manualRequirement: parts[index + 5] === "true",
    ciDefault: parts[index + 6] === "true",
    status: parts[index + 7],
    exitCode: Number(parts[index + 8]),
    durationMs: Number(parts[index + 9]),
    reason: parts[index + 10] || null,
    executionStatus: parts[index + 11],
    conditionStatus: parts[index + 12],
    startedAt: new Date(Number(parts[index + 13])).toISOString(),
    finishedAt: new Date(Number(parts[index + 14])).toISOString(),
  });
}

const payload = {
  schemaVersion: 2,
  generatedAt: new Date().toISOString(),
  workingDirectory,
  status: gates.some((gate) => gate.conditionStatus === "failed")
    ? "failed"
    : gates.some((gate) => gate.conditionStatus === "unknown") || gates.length < selectedGateCount
      ? "unknown"
      : "passed",
  selectedGateCount,
  executedGateCount: gates.length,
  failedGateCount: gates.filter((gate) => gate.status === "failed").length,
  failedExecutionCount: gates.filter((gate) => gate.executionStatus === "execution_failed").length,
  failedConditionCount: gates.filter((gate) => gate.conditionStatus === "failed").length,
  unknownConditionCount:
    gates.filter((gate) => gate.conditionStatus === "unknown").length +
    Math.max(0, selectedGateCount - gates.length),
  gates,
};
fs.writeFileSync(outputPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
' "$RESULTS_JSON"
}

add_gate 'acceptance matrix contract tests' 'node --test scripts/acceptance.test.mjs'
add_gate 'gitnexus fallback wrapper contract tests' 'node --test scripts/gitnexus-safe.test.mjs'
add_gate 'desktop production build' 'npm run build:desktop'
add_gate 'website production build' 'npm run build:website'
set_domain 'desktop-safety'
add_gate 'desktop deterministic signal cleanup' 'npm --prefix apps/desktop run check:deterministic-signals && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::handlers::tests::forgotten_memory_not_injected_via_select_context --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts --grep "continuity query stays console-clean"'
add_gate 'desktop command execution safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test harness_test project_defined -- --nocapture && npm --prefix apps/desktop run check:backend'
add_gate 'desktop credential and redaction safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_store --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml credential_migration --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml redaction --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml profile --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml log_store --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml logger --lib && npm --prefix apps/desktop run check:backend'
add_gate 'desktop checkpoint restore safety baseline' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::checkpoint_snapshot --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::project_checkpoint --lib'
add_gate 'desktop CSP and capability safety baseline' 'npm --prefix apps/desktop run check:security-config && npm --prefix apps/desktop run build && npm --prefix apps/desktop run tauri -- build --no-bundle'
set_domain 'foundation'
add_gate 'desktop frontend architecture' 'npm --prefix apps/desktop run check:frontend-architecture'
add_gate 'desktop protocol sync' 'npm --prefix apps/desktop run check:protocol'
set_domain 'eval'
add_gate 'eval runner test suite' 'npm run test:eval'
add_gate 'eval quality suite' 'cd apps/eval-runner && uv sync --frozen --dev && uv run pytest -q && uv run ruff check . && uv run ruff format --check . && uv run mypy app'
add_gate 'eval execution identity baseline' 'cd apps/eval-runner && uv run pytest tests/test_storage.py tests/test_runner.py tests/test_api.py tests/test_smoke.py -q -k "execution_identity or unknown_provider or queued_forge"'
add_gate 'eval independent workspace evidence baseline' 'cd apps/eval-runner && uv run pytest tests/test_workspace_observer.py tests/test_runner.py -q -k "workspace"'
add_gate 'eval trusted execution baseline' 'cd apps/eval-runner && uv run pytest tests/test_execution.py tests/test_cli.py tests/test_api.py tests/test_worker.py -q -k "trust"'
add_gate 'eval authenticated fenced worker baseline' 'cd apps/eval-runner && uv run pytest tests/test_api.py tests/test_storage.py tests/test_worker.py -q -k "auth or lease or stale"'
set_domain 'foundation'
add_gate 'release manifest contract validation' 'node --test scripts/validate-release-gate-profile.test.mjs scripts/validate-release-manifest.test.mjs'
set_domain 'eval'
add_gate 'release confidence summary contract tests' 'node --test scripts/release-confidence-summary.test.mjs && node scripts/release-confidence-summary.mjs --json >/dev/null && node scripts/release-confidence-summary.mjs --json --ci-default-only >/dev/null && node scripts/release-confidence-summary.mjs --help | rg -q "no-acceptance-matrix" && node scripts/release-confidence-summary.mjs --help | rg -q "out-dir" && rg -q "release confidence summary" CHANGELOG.md && rg -q "capability evidence" CHANGELOG.md && rg -q "verified capability evidence" CHANGELOG.md && rg -q "verified capability evidence" README.md && rg -q "verified capability evidence" apps/eval-runner/README.md && rg -q "ci-default-only" CHANGELOG.md && rg -q "ci-default-only" README.md && rg -q "ci-default-only" apps/eval-runner/README.md && rg -q "results-json" CHANGELOG.md && rg -q "results-json" README.md && rg -q "results-json" apps/eval-runner/README.md && rg -q "gate-results execution completeness" CHANGELOG.md && rg -q "gate-results execution completeness" README.md && rg -q "gate-results execution completeness" apps/eval-runner/README.md && rg -q "execution reason evidence" CHANGELOG.md && rg -q "execution reason evidence" README.md && rg -q "execution reason evidence" apps/eval-runner/README.md && rg -q "dashboard artifact output" CHANGELOG.md && rg -q "dashboard artifact output" README.md && rg -q "dashboard artifact output" apps/eval-runner/README.md && rg -q "acceptance domain/tier breakdowns" CHANGELOG.md && rg -q "acceptance domain/tier breakdowns" README.md && rg -q "acceptance domain/tier breakdowns" apps/eval-runner/README.md && rg -q "gate detail metadata" CHANGELOG.md && rg -q "gate detail metadata" README.md && rg -q "gate detail metadata" apps/eval-runner/README.md && rg -q "no-acceptance-matrix" CHANGELOG.md && rg -q "no-acceptance-matrix" README.md && rg -q "no-acceptance-matrix" apps/eval-runner/README.md && rg -q "fail-on-attention" CHANGELOG.md'
set_domain 'runtime'
add_gate 'loop event journal contract tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib'
add_gate 'projection rebuild/replay tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib'
add_gate 'policy preflight tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::policy --lib'
add_gate 'budget preflight tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib'
add_gate 'durable human gate tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib'
add_gate 'runtime health snapshot smoke' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runtime_health_snapshot_counts_loop_gateway_and_recovery_facts --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib && node --test apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts'
add_gate 'runtime authority fast gate' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runtime_health_snapshot_counts_loop_gateway_and_recovery_facts --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml recover_loop_task_marks_running_task_interrupted_and_recoverable --lib && node --test apps/desktop/src/lib/loopRuntime.test.ts apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts'
set_domain 'gateway'
add_gate 'gateway loop runner status smoke' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib'
add_gate 'gateway session-host run evidence' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib'
add_gate 'backend gateway restart smoke dry-run' 'npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run'
add_gate 'subagent runtime event projection smoke' 'node --test apps/desktop/src/store/blocks.test.ts'
add_gate 'live worktree worker lifecycle harness' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child::tests::run_worktree_worker --lib'
add_gate 'A2A child runtime event capsule and file IO bridge' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib && node --test apps/desktop/src/store/workbenchSummary.test.ts && rg -q "child runtime events" CHANGELOG.md && rg -q "child capsules" CHANGELOG.md && rg -q "review gate V2" CHANGELOG.md && rg -q "recovery suggestions" CHANGELOG.md'
add_gate 'executor file IO stream smoke' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib'
add_gate 'completion contract desktop helper smoke' 'node --test apps/desktop/src/lib/loopRuntime.test.ts'
set_domain 'ui-evidence'
add_gate 'completion contract mocked desktop smoke' 'npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts'
add_gate 'mocked desktop restart runtime smoke (partial macOS evidence)' 'npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts'
set_domain 'gateway'
add_gate 'desktop restart harness availability preflight' 'node scripts/desktop-restart-harness-preflight.mjs --json'
add_gate 'desktop restart harness preflight contract tests' 'node --test scripts/desktop-restart-harness-preflight.test.mjs'
add_gate 'desktop restart harness blocker documentation status' 'rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "node --test scripts/desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "blocked_official_macos" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md'
set_domain 'permission'
add_gate 'confirmation response replay contract tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "confirm response replay|startup transcript hydration"'
add_gate 'permission confirmation full access trust smoke specs' 'npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|confirmation|full access|trust"'
set_domain 'ui-evidence'
add_gate 'desktop UI evidence observer preflight' 'node scripts/desktop-ui-evidence-preflight.mjs --json'
add_gate 'desktop UI evidence doctor' 'node scripts/desktop-ui-evidence-doctor.mjs --json'
add_gate 'desktop UI evidence recovery checks' 'node scripts/desktop-ui-evidence-doctor.mjs --json --run-checks'
add_gate 'manual desktop restart smoke protocol' 'test -f apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "Stability Convergence Restart Smoke - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md'
add_gate 'manual stability regression batch' 'test -f apps/desktop/docs/product/stability-regression-batch.md && rg -q "Stability Regression Batch - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md'
add_gate 'manual disposable edit/build loop protocol' 'test -f apps/desktop/docs/product/phase8-disposable-loop-protocol.md && rg -q "Phase 8 Disposable Edit/Build Loop - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md'
add_gate 'disposable edit/build loop beta-log archive status' 'test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-1.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-2.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-3.validation.json && rg -q "Status: Archived complete for rows #1/#2/#3" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-1" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-2" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-3" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md'
add_gate 'disposable edit/build loop project readiness preflight' 'node scripts/disposable-loop-preflight.mjs --json'
add_gate 'disposable edit/build loop clean worktree prepare dry-run' 'node scripts/prepare-disposable-loop-project.mjs --json --dry-run'
add_gate 'disposable edit/build loop evidence collector' 'node scripts/collect-disposable-loop-evidence.mjs --json'
add_gate 'disposable edit/build loop evidence validator' 'node scripts/validate-disposable-loop-evidence.mjs --json'
add_gate 'disposable edit/build loop evidence archive dry-run' 'node scripts/archive-disposable-loop-evidence.mjs --json --dry-run'
add_gate 'disposable edit/build loop manual evidence template' 'node scripts/create-disposable-loop-manual-json.mjs --json --row 1'
add_gate 'disposable edit/build loop manual evidence review' 'node scripts/review-disposable-loop-manual-json.mjs --json --row 1'
add_gate 'disposable edit/build loop row finalizer dry-run' 'node scripts/finalize-disposable-loop-row.mjs --json --dry-run --row 1'
add_gate 'disposable edit/build loop row runbook' 'node scripts/phase8-disposable-loop-runbook.mjs --json --row 1'
add_gate 'disposable edit/build loop status summary' 'node scripts/phase8-disposable-loop-status.mjs --json'
add_gate 'disposable edit/build loop live-ready hard gate' 'node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready'
set_domain 'usage-context'
add_gate 'provider usage known/unknown telemetry' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib'
add_gate 'composer context usage from provider_usage' 'npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "provider_usage without legacy usage"'
add_gate 'provider usage trace rendering' 'npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "provider usage"'
add_gate 'legacy usage duplicate suppression' 'node --test apps/desktop/src/store/event-dispatch.test.ts'
add_gate 'transcript usage hydration' 'node --test apps/desktop/src/store/persistence-hydration.test.ts'
add_gate 'desktop state consistency map status' 'rg -q "provider_usage" docs/desktop/state-consistency-map.md && rg -q "Tauri transcript replay" docs/desktop/state-consistency-map.md && rg -q "transcript usage hydration" docs/desktop/state-consistency-map.md && rg -q "Gateway trigger run evidence" docs/desktop/state-consistency-map.md && rg -q "TriggerRunRecord" docs/desktop/state-consistency-map.md && rg -q "Gateway run state" docs/desktop/state-consistency-map.md && rg -q "smoke:gateway:restart" docs/desktop/state-consistency-map.md && rg -q "gateway::runner --lib" docs/desktop/state-consistency-map.md'
add_gate 'post-shell file-effect evidence smoke (bounded, not shell-internal)' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib'
add_gate 'persisted A2A lineage tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib'
add_gate 'typed completion evidence and review-to-commit eligibility tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib'
add_gate 'gated headless ownership policy tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib'
set_domain 'permission'
add_gate 'permission mode, live-session sync, and shell policy contract tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_handlers --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib'
add_gate 'slash command review calibration contract tests' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib'
set_domain 'ui-evidence'
add_gate 'desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs' 'npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts'
add_gate 'rich preview e2e smoke specs' 'npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "write_file tool details show|diff cards show|image diff cards show"'

if [[ "${#COMMAND_LABELS[@]}" -ne "${#COMMANDS[@]}" || "${#COMMAND_LABELS[@]}" -ne "${#GATE_DOMAINS[@]}" ]]; then
  echo "Acceptance matrix mismatch: ${#COMMAND_LABELS[@]} labels, ${#COMMANDS[@]} commands, ${#GATE_DOMAINS[@]} domains" >&2
  exit 1
fi

assert_unique() {
  local kind="$1"
  shift
  local values=("$@")
  local left_index
  local right_index

  for left_index in "${!values[@]}"; do
    for right_index in "${!values[@]}"; do
      if [[ "$right_index" -le "$left_index" ]]; then
        continue
      fi
      if [[ "${values[$left_index]}" == "${values[$right_index]}" ]]; then
        echo "Duplicate acceptance gate $kind: ${values[$left_index]}" >&2
        exit 1
      fi
    done
  done
}

assert_unique "label" "${COMMAND_LABELS[@]}"
assert_unique "command" "${COMMANDS[@]}"

domain_title() {
  case "$1" in
    foundation)
      printf 'Foundation'
      ;;
    desktop-safety)
      printf 'Desktop Safety'
      ;;
    runtime)
      printf 'Runtime'
      ;;
    permission)
      printf 'Permission'
      ;;
    usage-context)
      printf 'Usage / Context'
      ;;
    memory)
      printf 'Memory'
      ;;
    gateway)
      printf 'Gateway'
      ;;
    eval)
      printf 'Eval'
      ;;
    ui-evidence)
      printf 'UI Evidence'
      ;;
    *)
      printf '%s' "$1"
      ;;
  esac
}

manual_requirement() {
  local label="$1"
  [[ "$label" == manual\ * || "$label" == *"manual evidence"* || "$label" == *"beta-log archive"* ]]
}

gate_tier() {
  local domain="$1"
  local label="$2"
  local command="$3"

  if manual_requirement "$label"; then
    printf 'manual-evidence'
  elif [[ "$label" =~ production\ build || "$label" =~ eval\ runner\ test\ suite ]]; then
    printf 'full-release'
  elif [[ "$command" =~ e2e || "$command" =~ test:e2e ]]; then
    printf 'desktop-ui'
  elif [[ "$label" =~ contract || "$label" =~ preflight || "$label" =~ doctor || "$label" =~ summary || "$label" =~ template || "$label" =~ review || "$label" =~ dry-run || "$label" =~ list-json || "$label" =~ state\ consistency ]]; then
    printf 'fast-contract'
  elif [[ "$domain" == "ui-evidence" ]]; then
    printf 'desktop-ui'
  else
    printf 'runtime-core'
  fi
}

ci_default_tier() {
  [[ "$1" == "fast-contract" || "$1" == "runtime-core" ]]
}

runtime_cost() {
  local label="$1"
  local command="$2"

  if [[ "$command" =~ production\ build || "$command" =~ test:eval || "$command" =~ test:e2e || "$command" =~ uv\ run\ pytest\ tests/test_cases.py ]]; then
    printf 'long'
  elif [[ "$command" =~ cargo\ test || "$command" =~ uv\ run\ pytest || "$command" =~ node\ --test ]]; then
    if [[ "$command" == *"&&"* || "$label" =~ runtime\ authority\ fast\ gate || "$label" =~ desktop\ eval ]]; then
      printf 'medium'
    else
      printf 'short'
    fi
  else
    printf 'short'
  fi
}

RELEASE_LABELS=()
if [[ -n "$RELEASE_PROFILE" ]]; then
  if [[ "$RELEASE_PROFILE" != /* ]]; then
    RELEASE_PROFILE="$ROOT_DIR/$RELEASE_PROFILE"
  fi
  node "$ROOT_DIR/scripts/validate-release-gate-profile.mjs" \
    --release-profile "$RELEASE_PROFILE" \
    --require-state "$REQUIRE_STATE" >/dev/null
  while IFS= read -r label; do
    RELEASE_LABELS+=("$label")
  done < <(node -e '
const fs = require("node:fs");
const profile = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const state = process.argv[2];
for (const gate of profile.gates ?? []) {
  const states = Array.isArray(gate.required_for) ? gate.required_for : [];
  if (states.includes(state) || (state === "R4" && states.includes("R3"))) {
    process.stdout.write(`${gate.label}\n`);
  }
}
' "$RELEASE_PROFILE" "$REQUIRE_STATE")
fi

release_label_selected() {
  local candidate="$1"
  local required_label
  for required_label in "${RELEASE_LABELS[@]}"; do
    if [[ "$candidate" == "$required_label" ]]; then
      return 0
    fi
  done
  return 1
}

SELECTED_INDICES=()
for index in "${!COMMANDS[@]}"; do
  if [[ -n "$ONLY_LABEL" && "${COMMAND_LABELS[$index]}" != "$ONLY_LABEL" ]]; then
    continue
  fi
  if [[ -n "$GREP_LABEL" ]]; then
    label_match="$(printf '%s' "${COMMAND_LABELS[$index]}" | tr '[:upper:]' '[:lower:]')"
    if [[ "$label_match" != *"$GREP_LABEL_MATCH"* ]]; then
      continue
    fi
  fi
  if [[ "$CI_DEFAULT" -eq 1 ]]; then
    tier="$(gate_tier "${GATE_DOMAINS[$index]}" "${COMMAND_LABELS[$index]}" "${COMMANDS[$index]}")"
    if ! ci_default_tier "$tier"; then
      continue
    fi
  fi
  if [[ -n "$RELEASE_PROFILE" ]] && ! release_label_selected "${COMMAND_LABELS[$index]}"; then
    continue
  fi
  SELECTED_INDICES+=("$index")
done

if [[ -n "$RELEASE_PROFILE" ]]; then
  PROFILE_ORDERED_INDICES=()
  for required_label in "${RELEASE_LABELS[@]}"; do
    for index in "${SELECTED_INDICES[@]}"; do
      if [[ "${COMMAND_LABELS[$index]}" == "$required_label" ]]; then
        PROFILE_ORDERED_INDICES+=("$index")
        break
      fi
    done
  done
  SELECTED_INDICES=("${PROFILE_ORDERED_INDICES[@]}")
fi

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

if [[ -n "$GREP_LABEL" && "${#SELECTED_INDICES[@]}" -eq 0 ]]; then
  echo "No acceptance gates matched --grep: $GREP_LABEL" >&2
  echo "Run scripts/acceptance.sh --list-json to see valid labels." >&2
  exit 2
fi

if [[ "$CI_DEFAULT" -eq 1 && "${#SELECTED_INDICES[@]}" -eq 0 ]]; then
  echo "No acceptance gates matched --ci-default." >&2
  echo "Run scripts/acceptance.sh --list-json to inspect gate tiers." >&2
  exit 2
fi

if [[ -n "$RELEASE_PROFILE" && "${#SELECTED_INDICES[@]}" -ne "${#RELEASE_LABELS[@]}" ]]; then
  echo "Release profile labels do not match the acceptance matrix for $REQUIRE_STATE." >&2
  exit 2
fi

if [[ "$SHOW_HELP" -eq 1 ]]; then
  cat <<'EOF'
Usage: scripts/acceptance.sh [--dry-run] [--list-json] [--only <label>] [--grep <text>] [--ci-default] [--release-profile <path> --require-state <R1|R2|R3|R4>] [--results-json <path>]

Runs the Forge Level 3 runtime acceptance gates:
EOF
  previous_domain=""
  for index in "${!COMMAND_LABELS[@]}"; do
    domain="${GATE_DOMAINS[$index]}"
    if [[ "$domain" != "$previous_domain" ]]; then
      printf '\n  %s:\n' "$(domain_title "$domain")"
      previous_domain="$domain"
    fi
    printf '    %s. %s\n' "$((index + 1))" "${COMMAND_LABELS[$index]}"
  done
  cat <<'EOF'

Use --dry-run to print the command plan without executing it.
Use --list-json to print the same gate plan as machine-readable JSON.
Use --only with an exact gate label to run or dry-run one gate.
Use --grep to filter gates by case-insensitive label substring.
Use --ci-default to run or list only fast-contract and runtime-core gates.
Use --release-profile with --require-state to run or list the exact fail-closed release gate set.
Use --results-json to write executed gate statuses for release confidence reports.
Do not combine --only, --grep, --ci-default, and --release-profile.
EOF
  exit 0
fi

if [[ "$LIST_JSON" -eq 1 ]]; then
  {
    printf '%s\0' "$ROOT_DIR"
    for index in "${SELECTED_INDICES[@]}"; do
      printf '%s\0%s\0%s\0%s\0' "$((index + 1))" "${GATE_DOMAINS[$index]}" "${COMMAND_LABELS[$index]}" "${COMMANDS[$index]}"
    done
  } | node -e '
const fs = require("node:fs");
const parts = fs.readFileSync(0).toString("utf8").split("\0");
const workingDirectory = parts.shift();
if (parts.at(-1) === "") parts.pop();
const domainLabels = new Map([
  ["foundation", "Foundation"],
  ["desktop-safety", "Desktop Safety"],
  ["runtime", "Runtime"],
  ["permission", "Permission"],
  ["usage-context", "Usage / Context"],
  ["memory", "Memory"],
  ["gateway", "Gateway"],
  ["eval", "Eval"],
  ["ui-evidence", "UI Evidence"],
]);
const tierLabels = new Map([
  ["fast-contract", "Fast Contract"],
  ["runtime-core", "Runtime Core"],
  ["desktop-ui", "Desktop UI"],
  ["manual-evidence", "Manual Evidence"],
  ["full-release", "Full Release"],
]);

function gateTier({ domain, label, command }) {
  if (manualRequirement(label)) return "manual-evidence";
  if (/production build|eval runner test suite/.test(label)) return "full-release";
  if (/e2e|test:e2e/.test(command)) return "desktop-ui";
  if (
    /contract|preflight|doctor|summary|template|review|dry-run|list-json|state consistency/.test(
      label,
    )
  ) {
    return "fast-contract";
  }
  if (domain === "ui-evidence") return "desktop-ui";
  return "runtime-core";
}

function runtimeCost({ label, command }) {
  if (/production build|test:eval|test:e2e|uv run pytest tests\/test_cases.py/.test(command)) {
    return "long";
  }
  if (/cargo test|uv run pytest|node --test/.test(command)) {
    return /&&/.test(command) || /runtime authority fast gate|desktop eval/.test(label)
      ? "medium"
      : "short";
  }
  return "short";
}

function manualRequirement(label) {
  return /^manual /.test(label) || /manual evidence|beta-log archive/.test(label);
}

const gates = [];
for (let index = 0; index < parts.length; index += 4) {
  const gate = {
    index: Number(parts[index]),
    domain: parts[index + 1],
    label: parts[index + 2],
    command: parts[index + 3],
  };
  gate.tier = gateTier(gate);
  gate.runtimeCost = runtimeCost(gate);
  gate.manualRequirement = manualRequirement(gate.label);
  gate.ciDefault = gate.tier === "fast-contract" || gate.tier === "runtime-core";
  gates.push({
    ...gate,
  });
}
const domains = [...domainLabels].map(([id, label]) => ({
  id,
  label,
  gateCount: gates.filter((gate) => gate.domain === id).length,
}));
const tiers = [...tierLabels].map(([id, label]) => ({
  id,
  label,
  ciDefault: id === "fast-contract" || id === "runtime-core",
  gateCount: gates.filter((gate) => gate.tier === id).length,
  manualGateCount: gates.filter((gate) => gate.tier === id && gate.manualRequirement).length,
}));
process.stdout.write(JSON.stringify({ schemaVersion: 1, workingDirectory, domains, tiers, gates }, null, 2) + "\n");
'
  exit 0
fi

echo "Forge Level 3 runtime acceptance suite"
echo "Working directory: $ROOT_DIR"
echo

overall_exit_code=0
for index in "${SELECTED_INDICES[@]}"; do
  label="${COMMAND_LABELS[$index]}"
  command="${COMMANDS[$index]}"
  domain="${GATE_DOMAINS[$index]}"
  tier="$(gate_tier "$domain" "$label" "$command")"
  cost="$(runtime_cost "$label" "$command")"
  if manual_requirement "$label"; then
    manual="true"
  else
    manual="false"
  fi
  if ci_default_tier "$tier"; then
    ci_default="true"
  else
    ci_default="false"
  fi
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $label: $command"
  else
    echo "==> $label"
    echo "    $command"
    started_at_ms="$(now_ms)"
    set +e
    (cd "$ROOT_DIR" && eval "$command")
    gate_exit_code="$?"
    set -e
    finished_at_ms="$(now_ms)"
    duration_ms=$((finished_at_ms - started_at_ms))
    record_gate_result "$label" "$command" "$domain" "$tier" "$cost" "$manual" "$ci_default" "$gate_exit_code" "$duration_ms" "$started_at_ms" "$finished_at_ms"
    if [[ "$gate_exit_code" -ne 0 ]]; then
      overall_exit_code="$gate_exit_code"
      break
    fi
  fi
done

write_results_json "$overall_exit_code"
exit "$overall_exit_code"
