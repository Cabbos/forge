import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "acceptance.sh");
const a2aLineageCommand = [
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib",
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib",
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib",
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib",
  "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib",
].join(" && ");

function parseDryRunEntries(output) {
  return output
    .split(/\r?\n/)
    .filter((line) => line.startsWith("[dry-run] "))
    .map((line) => {
      const match = line.match(/^\[dry-run\] ([^:]+): (.+)$/);
      assert.ok(match, `dry-run line should include label and command: ${line}`);
      return { label: match[1], command: match[2] };
    });
}

function parseHelpGateEntries(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.match(/^\s+(\d+)\. (.+)$/))
    .filter(Boolean)
    .map((match) => ({ index: Number(match[1]), label: match[2] }));
}

test("acceptance script dry-run lists the final product gates", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(scriptPath, ["--dry-run"], {
    cwd: root,
    encoding: "utf8",
  });

  const dryRunEntries = parseDryRunEntries(output);
  const expectedEntries = [
    { label: "acceptance matrix contract tests", command: "node --test scripts/acceptance.test.mjs" },
    { label: "gitnexus fallback wrapper contract tests", command: "node --test scripts/gitnexus-safe.test.mjs" },
    { label: "desktop production build", command: "npm run build:desktop" },
    { label: "website production build", command: "npm run build:website" },
    {
      label: "desktop deterministic signal cleanup",
      command:
        'tmp="${TMPDIR:-/tmp}/forge-desktop-build.$$"; npm --prefix apps/desktop run build >"$tmp" 2>&1; code=$?; cat "$tmp"; if [ "$code" -ne 0 ]; then rm -f "$tmp"; exit "$code"; fi; if rg -qi "Unknown at rule|@theme|@utility|@custom-variant" "$tmp"; then rm -f "$tmp"; exit 1; fi; rm -f "$tmp"; cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::handlers::tests::forgotten_memory_not_injected_via_select_context --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts --grep "continuity query stays console-clean"',
    },
    { label: "eval runner test suite", command: "npm run test:eval" },
    {
      label: "release confidence summary contract tests",
      command:
        'node --test scripts/release-confidence-summary.test.mjs && node scripts/release-confidence-summary.mjs --json >/dev/null && node scripts/release-confidence-summary.mjs --json --ci-default-only >/dev/null && node scripts/release-confidence-summary.mjs --help | rg -q "no-acceptance-matrix" && node scripts/release-confidence-summary.mjs --help | rg -q "out-dir" && rg -q "release confidence summary" CHANGELOG.md && rg -q "capability evidence" CHANGELOG.md && rg -q "verified capability evidence" CHANGELOG.md && rg -q "verified capability evidence" README.md && rg -q "verified capability evidence" apps/eval-runner/README.md && rg -q "ci-default-only" CHANGELOG.md && rg -q "ci-default-only" README.md && rg -q "ci-default-only" apps/eval-runner/README.md && rg -q "results-json" CHANGELOG.md && rg -q "results-json" README.md && rg -q "results-json" apps/eval-runner/README.md && rg -q "gate-results execution completeness" CHANGELOG.md && rg -q "gate-results execution completeness" README.md && rg -q "gate-results execution completeness" apps/eval-runner/README.md && rg -q "execution reason evidence" CHANGELOG.md && rg -q "execution reason evidence" README.md && rg -q "execution reason evidence" apps/eval-runner/README.md && rg -q "dashboard artifact output" CHANGELOG.md && rg -q "dashboard artifact output" README.md && rg -q "dashboard artifact output" apps/eval-runner/README.md && rg -q "acceptance domain/tier breakdowns" CHANGELOG.md && rg -q "acceptance domain/tier breakdowns" README.md && rg -q "acceptance domain/tier breakdowns" apps/eval-runner/README.md && rg -q "gate detail metadata" CHANGELOG.md && rg -q "gate detail metadata" README.md && rg -q "gate detail metadata" apps/eval-runner/README.md && rg -q "no-acceptance-matrix" CHANGELOG.md && rg -q "no-acceptance-matrix" README.md && rg -q "no-acceptance-matrix" apps/eval-runner/README.md && rg -q "fail-on-attention" CHANGELOG.md',
    },
    {
      label: "loop event journal contract tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib",
    },
    {
      label: "projection rebuild/replay tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib",
    },
    {
      label: "policy preflight tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::policy --lib",
    },
    {
      label: "budget preflight tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib",
    },
    {
      label: "durable human gate tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib",
    },
    {
      label: "runtime health snapshot smoke",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runtime_health_snapshot_counts_loop_gateway_and_recovery_facts --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib && node --test apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts",
    },
    {
      label: "runtime authority fast gate",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runtime_health_snapshot_counts_loop_gateway_and_recovery_facts --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml recover_loop_task_marks_running_task_interrupted_and_recoverable --lib && node --test apps/desktop/src/lib/loopRuntime.test.ts apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts",
    },
    {
      label: "gateway loop runner status smoke",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib",
    },
    {
      label: "gateway session-host run evidence",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib",
    },
    {
      label: "backend gateway restart smoke dry-run",
      command: "npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run",
    },
    {
      label: "desktop eval promotion evidence smoke",
      command:
        'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin forge_session build_session_eval_trace_payload_uses_snapshot_and_transcript_facts && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml trace_payload_maps_forge_events_turn_state_and_diffs --lib && cd apps/eval-runner && uv run pytest tests/test_cases.py tests/test_metrics.py tests/test_runner.py -q && cd ../.. && rg -q "A2A child evidence completeness" CHANGELOG.md && rg -q "A2A eval pack" CHANGELOG.md && rg -q "context capsule contract" CHANGELOG.md && rg -q "context capsule contract" apps/eval-runner/README.md && rg -q "review gate identity" CHANGELOG.md && rg -q "review gate identity" apps/eval-runner/README.md && rg -q "blocking review gates" CHANGELOG.md && rg -q "blocking review gates" apps/eval-runner/README.md && rg -q "failure recovery policy" CHANGELOG.md && rg -q "failure recovery policy" apps/eval-runner/README.md && rg -q "runtime event file facts" CHANGELOG.md && rg -q "runtime event file facts" apps/eval-runner/README.md && rg -q "worktree worker facts" CHANGELOG.md && rg -q "worktree worker facts" apps/eval-runner/README.md && rg -q "gateway runtime safety" CHANGELOG.md && rg -q "gateway eval pack" CHANGELOG.md && rg -q "direct-write owner blocked" CHANGELOG.md && rg -q "direct-write owner blocked" apps/eval-runner/README.md && rg -q "lease timeout recovery" CHANGELOG.md && rg -q "duplicate input prevention" CHANGELOG.md && rg -q "runtime recovery quality" CHANGELOG.md && rg -q "runtime recovery eval pack" CHANGELOG.md && rg -q "ForgeRunEvidence V2" CHANGELOG.md && rg -q "completion eligibility evidence scoring" CHANGELOG.md && rg -q "context budget bucket scoring" CHANGELOG.md && rg -q "context budget bucket scoring" apps/eval-runner/README.md && rg -q "schema identity scoring" CHANGELOG.md && rg -q "schema identity scoring" apps/eval-runner/README.md && rg -q "permission decision evidence scoring" CHANGELOG.md && rg -q "permission decision evidence scoring" apps/eval-runner/README.md && rg -q "verification evidence quality scoring" CHANGELOG.md && rg -q "verification evidence quality scoring" apps/eval-runner/README.md && rg -q "usage unknown conflict scoring" CHANGELOG.md && rg -q "usage unknown conflict scoring" apps/eval-runner/README.md && rg -q "provider usage value validation" CHANGELOG.md && rg -q "provider usage value validation" apps/eval-runner/README.md && rg -q "prepared-turn evidence scoring" CHANGELOG.md && rg -q "prepared-turn evidence scoring" apps/eval-runner/README.md && rg -q "file effects evidence scoring" CHANGELOG.md && rg -q "file effects evidence scoring" apps/eval-runner/README.md && rg -q "tool/shell evidence scoring" CHANGELOG.md && rg -q "tool/shell evidence scoring" apps/eval-runner/README.md && rg -q "failure evidence scoring" CHANGELOG.md && rg -q "failure evidence scoring" apps/eval-runner/README.md && rg -q "continuity lessons scoring" CHANGELOG.md && rg -q "continuity lessons scoring" apps/eval-runner/README.md && rg -q "memory recall audit scoring" CHANGELOG.md && rg -q "memory recall audit scoring" apps/eval-runner/README.md',
    },
    {
      label: "memory recall and archive coverage status",
      command:
        'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml memory::unified --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::unified_memory --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::send_input_context --lib && cd apps/eval-runner && uv run pytest tests/test_cases.py tests/test_metrics.py -q && cd ../.. && rg -q "Project Archive unified memory" CHANGELOG.md && rg -q "recall audit" CHANGELOG.md && rg -q "context budget buckets" CHANGELOG.md && rg -q "memory recall quality" CHANGELOG.md && rg -q "memory eval pack" CHANGELOG.md && rg -q "memory/continuity dedupe" CHANGELOG.md && rg -q "memory/continuity dedupe" apps/eval-runner/README.md && rg -q "project archive shows unified memory overview and search" apps/desktop/e2e/acceptance.spec.ts',
    },
    {
      label: "memory physical migration dry-run report",
      command:
        'node --test scripts/memory-migration-dry-run.test.mjs && node scripts/memory-migration-dry-run.mjs --json >/dev/null && rg -q "memory physical migration dry-run" CHANGELOG.md && rg -q "physical store migration dry-run" README.md && rg -q "physical store migration dry-run" apps/desktop/README.md',
    },
    {
      label: "subagent runtime event projection smoke",
      command: "node --test apps/desktop/src/store/blocks.test.ts",
    },
    {
      label: "live worktree worker lifecycle harness",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child::tests::run_worktree_worker --lib",
    },
    {
      label: "A2A child runtime event capsule and file IO bridge",
      command:
        'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib && node --test apps/desktop/src/store/workbenchSummary.test.ts && rg -q "child runtime events" CHANGELOG.md && rg -q "child capsules" CHANGELOG.md && rg -q "review gate V2" CHANGELOG.md && rg -q "recovery suggestions" CHANGELOG.md',
    },
    {
      label: "executor file IO stream smoke",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib",
    },
    {
      label: "completion contract desktop helper smoke",
      command: "node --test apps/desktop/src/lib/loopRuntime.test.ts",
    },
    {
      label: "completion contract mocked desktop smoke",
      command: "npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts",
    },
    {
      label: "mocked desktop restart runtime smoke (partial macOS evidence)",
      command: "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts",
    },
    {
      label: "desktop restart harness availability preflight",
      command: "node scripts/desktop-restart-harness-preflight.mjs --json",
    },
    {
      label: "desktop restart harness preflight contract tests",
      command: "node --test scripts/desktop-restart-harness-preflight.test.mjs",
    },
    {
      label: "desktop restart harness blocker documentation status",
      command:
        'rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "node --test scripts/desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "blocked_official_macos" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
    {
      label: "confirmation response replay contract tests",
      command:
        'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "confirm response replay|startup transcript hydration"',
    },
    {
      label: "permission confirmation full access trust smoke specs",
      command: 'npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|confirmation|full access|trust"',
    },
    {
      label: "desktop UI evidence observer preflight",
      command: "node scripts/desktop-ui-evidence-preflight.mjs --json",
    },
    {
      label: "desktop UI evidence doctor",
      command: "node scripts/desktop-ui-evidence-doctor.mjs --json",
    },
    {
      label: "desktop UI evidence recovery checks",
      command: "node scripts/desktop-ui-evidence-doctor.mjs --json --run-checks",
    },
    {
      label: "manual desktop restart smoke protocol",
      command:
        'test -f apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "Stability Convergence Restart Smoke - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
    {
      label: "manual stability regression batch",
      command:
        'test -f apps/desktop/docs/product/stability-regression-batch.md && rg -q "Stability Regression Batch - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
    {
      label: "manual disposable edit/build loop protocol",
      command:
        'test -f apps/desktop/docs/product/phase8-disposable-loop-protocol.md && rg -q "Phase 8 Disposable Edit/Build Loop - 2026-06-27" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
    {
      label: "disposable edit/build loop beta-log archive status",
      command:
        'test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-1.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-2.validation.json && test -f apps/desktop/docs/product/evidence/phase8-disposable-loop/2026-06-28-row-3.validation.json && rg -q "Status: Archived complete for rows #1/#2/#3" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-1" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-2" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "2026-06-28-row-3" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
    {
      label: "disposable edit/build loop project readiness preflight",
      command: "node scripts/disposable-loop-preflight.mjs --json",
    },
    {
      label: "disposable edit/build loop clean worktree prepare dry-run",
      command: "node scripts/prepare-disposable-loop-project.mjs --json --dry-run",
    },
    {
      label: "disposable edit/build loop evidence collector",
      command: "node scripts/collect-disposable-loop-evidence.mjs --json",
    },
    {
      label: "disposable edit/build loop evidence validator",
      command: "node scripts/validate-disposable-loop-evidence.mjs --json",
    },
    {
      label: "disposable edit/build loop evidence archive dry-run",
      command: "node scripts/archive-disposable-loop-evidence.mjs --json --dry-run",
    },
    {
      label: "disposable edit/build loop manual evidence template",
      command: "node scripts/create-disposable-loop-manual-json.mjs --json --row 1",
    },
    {
      label: "disposable edit/build loop manual evidence review",
      command: "node scripts/review-disposable-loop-manual-json.mjs --json --row 1",
    },
    {
      label: "disposable edit/build loop row finalizer dry-run",
      command: "node scripts/finalize-disposable-loop-row.mjs --json --dry-run --row 1",
    },
    {
      label: "disposable edit/build loop row runbook",
      command: "node scripts/phase8-disposable-loop-runbook.mjs --json --row 1",
    },
    {
      label: "disposable edit/build loop status summary",
      command: "node scripts/phase8-disposable-loop-status.mjs --json",
    },
    {
      label: "disposable edit/build loop live-ready hard gate",
      command: "node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready",
    },
    {
      label: "provider usage known/unknown telemetry",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib",
    },
    {
      label: "composer context usage from provider_usage",
      command: 'npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "provider_usage without legacy usage"',
    },
    {
      label: "provider usage trace rendering",
      command: 'npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "provider usage"',
    },
    {
      label: "legacy usage duplicate suppression",
      command: "node --test apps/desktop/src/store/event-dispatch.test.ts",
    },
    {
      label: "transcript usage hydration",
      command: "node --test apps/desktop/src/store/persistence-hydration.test.ts",
    },
    {
      label: "desktop state consistency map status",
      command:
        'rg -q "provider_usage" docs/desktop/state-consistency-map.md && rg -q "Tauri transcript replay" docs/desktop/state-consistency-map.md && rg -q "transcript usage hydration" docs/desktop/state-consistency-map.md && rg -q "Gateway trigger run evidence" docs/desktop/state-consistency-map.md && rg -q "TriggerRunRecord" docs/desktop/state-consistency-map.md && rg -q "Gateway run state" docs/desktop/state-consistency-map.md && rg -q "smoke:gateway:restart" docs/desktop/state-consistency-map.md && rg -q "gateway::runner --lib" docs/desktop/state-consistency-map.md',
    },
    {
      label: "post-shell file-effect evidence smoke (bounded, not shell-internal)",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib",
    },
    {
      label: "persisted A2A lineage tests",
      command: a2aLineageCommand,
    },
    {
      label: "typed completion evidence and review-to-commit eligibility tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib",
    },
    {
      label: "gated headless ownership policy tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib",
    },
    {
      label: "permission mode, live-session sync, and shell policy contract tests",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_handlers --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib",
    },
    {
      label: "slash command review calibration contract tests",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib",
    },
    {
      label: "desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs",
      command:
        "npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts",
    },
    {
      label: "rich preview e2e smoke specs",
      command:
        'npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "write_file tool details show|diff cards show|image diff cards show"',
    },
  ];

  assert.deepEqual(dryRunEntries, expectedEntries);

  const ownershipGateOrder = [
    "completion contract mocked desktop smoke",
    "mocked desktop restart runtime smoke (partial macOS evidence)",
    "desktop restart harness availability preflight",
    "desktop restart harness preflight contract tests",
    "desktop restart harness blocker documentation status",
    "confirmation response replay contract tests",
    "permission confirmation full access trust smoke specs",
    "desktop UI evidence observer preflight",
    "desktop UI evidence doctor",
    "desktop UI evidence recovery checks",
    "manual desktop restart smoke protocol",
    "manual stability regression batch",
    "manual disposable edit/build loop protocol",
    "disposable edit/build loop beta-log archive status",
    "disposable edit/build loop project readiness preflight",
    "disposable edit/build loop clean worktree prepare dry-run",
    "disposable edit/build loop evidence collector",
    "disposable edit/build loop evidence validator",
    "disposable edit/build loop evidence archive dry-run",
    "disposable edit/build loop manual evidence template",
    "disposable edit/build loop manual evidence review",
    "disposable edit/build loop row finalizer dry-run",
    "disposable edit/build loop row runbook",
    "disposable edit/build loop status summary",
    "disposable edit/build loop live-ready hard gate",
    "provider usage known/unknown telemetry",
    "composer context usage from provider_usage",
    "provider usage trace rendering",
    "legacy usage duplicate suppression",
    "transcript usage hydration",
    "desktop state consistency map status",
    "post-shell file-effect evidence smoke (bounded, not shell-internal)",
    "persisted A2A lineage tests",
    "typed completion evidence and review-to-commit eligibility tests",
    "gated headless ownership policy tests",
    "permission mode, live-session sync, and shell policy contract tests",
    "slash command review calibration contract tests",
  ];
  let previousIndex = -1;
  for (const label of ownershipGateOrder) {
    const index = output.indexOf(`[dry-run] ${label}:`);
    assert.notEqual(index, -1, `expected dry-run label: ${label}`);
    assert.ok(index > previousIndex, `expected ${label} after previous ownership gate`);
    previousIndex = index;
  }

  assert.doesNotMatch(output, /\[dry-run\] typed completion evidence tests:/);
  const ownershipEntries = dryRunEntries.filter(({ label }) => ownershipGateOrder.includes(label));
  const ownershipCommands = ownershipEntries.map(({ command }) => command);
  assert.equal(new Set(ownershipCommands).size, ownershipCommands.length, "ownership gate commands must be unique");

  for (const command of ownershipCommands) {
    assert.equal(
      dryRunEntries.filter((entry) => entry.command === command).length,
      1,
      `ownership command should appear once in the matrix: ${command}`,
    );
  }

  const ownershipSubcommands = ownershipCommands.flatMap((command) => command.split(/\s+&&\s+/));
  assert.equal(
    new Set(ownershipSubcommands).size,
    ownershipSubcommands.length,
    "ownership gate subcommands must be unique",
  );
});

test("acceptance script exposes a machine-readable gate list", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const dryRunOutput = execFileSync(scriptPath, ["--dry-run"], {
    cwd: root,
    encoding: "utf8",
  });
  const listOutput = execFileSync(scriptPath, ["--list-json"], {
    cwd: root,
    encoding: "utf8",
  });

  const dryRunEntries = parseDryRunEntries(dryRunOutput);
  const matrix = JSON.parse(listOutput);

  assert.equal(matrix.schemaVersion, 1);
  assert.equal(matrix.workingDirectory, root.replace(/\/$/, ""));
  assert.ok(Array.isArray(matrix.domains));
  assert.deepEqual(
    matrix.gates.map(({ index, label, command }) => ({ index, label, command })),
    dryRunEntries.map((entry, index) => ({
      index: index + 1,
      ...entry,
    })),
  );
  assert.ok(matrix.gates.every((gate) => typeof gate.domain === "string" && gate.domain.length > 0));
  assert.ok(matrix.gates.every((gate) => typeof gate.tier === "string" && gate.tier.length > 0));
  assert.ok(
    matrix.gates.every(
      (gate) => typeof gate.runtimeCost === "string" && gate.runtimeCost.length > 0,
    ),
  );
  assert.ok(
    matrix.gates.every((gate) => typeof gate.manualRequirement === "boolean"),
  );
  assert.ok(matrix.gates.every((gate) => typeof gate.ciDefault === "boolean"));
  assert.equal(
    matrix.gates.find(({ label }) => label === "runtime authority fast gate").ciDefault,
    true,
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "desktop production build").ciDefault,
    false,
  );
});

test("acceptance script annotates gates with backend authority domains", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const matrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json"], {
      cwd: root,
      encoding: "utf8",
    }),
  );

  assert.deepEqual(
    matrix.domains.map(({ id }) => id),
    ["foundation", "runtime", "permission", "usage-context", "memory", "gateway", "eval", "ui-evidence"],
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "runtime journal authority and recovery smoke").domain,
    "runtime",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "runtime health snapshot smoke").domain,
    "runtime",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "permission mode, live-session sync, and shell policy contract tests").domain,
    "permission",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "provider usage known/unknown telemetry").domain,
    "usage-context",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "memory recall and archive coverage status").domain,
    "memory",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "memory physical migration dry-run report").domain,
    "memory",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "gateway loop runner status smoke").domain,
    "gateway",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "gateway parity and degraded fallback smoke").domain,
    "gateway",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "desktop eval promotion evidence smoke").domain,
    "eval",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "desktop UI evidence observer preflight").domain,
    "ui-evidence",
  );
  assert.equal(matrix.domains.find(({ id }) => id === "memory").gateCount, 2);
});

test("acceptance script annotates gates with release tiers and manual requirements", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const matrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json"], {
      cwd: root,
      encoding: "utf8",
    }),
  );

  assert.deepEqual(
    matrix.tiers.map(({ id }) => id),
    ["fast-contract", "runtime-core", "desktop-ui", "manual-evidence", "full-release"],
  );
  assert.deepEqual(
    matrix.tiers.filter(({ ciDefault }) => ciDefault).map(({ id }) => id),
    ["fast-contract", "runtime-core"],
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "acceptance matrix contract tests").tier,
    "fast-contract",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "runtime authority fast gate").tier,
    "runtime-core",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "completion contract mocked desktop smoke").tier,
    "desktop-ui",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "manual desktop restart smoke protocol").tier,
    "manual-evidence",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "desktop production build").tier,
    "full-release",
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "manual desktop restart smoke protocol")
      .manualRequirement,
    true,
  );
  assert.equal(
    matrix.gates.find(({ label }) => label === "runtime authority fast gate").manualRequirement,
    false,
  );
});

test("acceptance help lists the same gates as list-json", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const helpOutput = execFileSync(scriptPath, ["--help"], {
    cwd: root,
    encoding: "utf8",
  });
  const matrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json"], {
      cwd: root,
      encoding: "utf8",
    }),
  );

  assert.deepEqual(
    parseHelpGateEntries(helpOutput),
    matrix.gates.map(({ index, label }) => ({ index, label })),
  );
  assert.match(helpOutput, /--results-json <path>/);
});

test("acceptance script can dry-run one named gate", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(
    scriptPath,
    ["--dry-run", "--only", "desktop restart harness blocker documentation status"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.deepEqual(parseDryRunEntries(output), [
    {
      label: "desktop restart harness blocker documentation status",
      command:
        'rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "node --test scripts/desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/desktop-restart-smoke-protocol.md && rg -q "blocked_official_macos" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "official macOS WKWebView WebDriver support" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop restart harness launch command" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md && rg -q "desktop-restart-harness-preflight.test.mjs" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md',
    },
  ]);
});

test("acceptance script can dry-run gates by case-insensitive label substring", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(scriptPath, ["--dry-run", "--grep", "Provider"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.deepEqual(parseDryRunEntries(output), [
    {
      label: "provider usage known/unknown telemetry",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib",
    },
    {
      label: "composer context usage from provider_usage",
      command: 'npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "provider_usage without legacy usage"',
    },
    {
      label: "provider usage trace rendering",
      command: 'npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "provider usage"',
    },
  ]);
});

test("runtime grep includes the runtime authority fast gate", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(scriptPath, ["--dry-run", "--grep", "runtime"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.match(output, /\[dry-run\] runtime authority fast gate:/);
});

test("acceptance script list-json uses the same grep-filtered gates as dry-run", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const dryRunOutput = execFileSync(scriptPath, ["--dry-run", "--grep", "Provider"], {
    cwd: root,
    encoding: "utf8",
  });
  const matrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json", "--grep", "Provider"], {
      cwd: root,
      encoding: "utf8",
    }),
  );

  assert.equal(matrix.schemaVersion, 1);
  assert.deepEqual(
    matrix.gates.map(({ label, command }) => ({ label, command })),
    parseDryRunEntries(dryRunOutput),
  );
});

test("acceptance script can dry-run and list CI-default gates", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const fullMatrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json"], {
      cwd: root,
      encoding: "utf8",
    }),
  );
  const ciDefaultLabels = fullMatrix.gates
    .filter(({ ciDefault }) => ciDefault)
    .map(({ label }) => label);

  const dryRunOutput = execFileSync(scriptPath, ["--dry-run", "--ci-default"], {
    cwd: root,
    encoding: "utf8",
  });
  const ciMatrix = JSON.parse(
    execFileSync(scriptPath, ["--list-json", "--ci-default"], {
      cwd: root,
      encoding: "utf8",
    }),
  );

  assert.ok(ciDefaultLabels.length > 0, "fixture should expose CI-default gates");
  assert.ok(ciDefaultLabels.includes("runtime authority fast gate"));
  assert.ok(!ciDefaultLabels.includes("desktop production build"));
  assert.deepEqual(
    parseDryRunEntries(dryRunOutput).map(({ label }) => label),
    ciDefaultLabels,
  );
  assert.deepEqual(
    ciMatrix.gates.map(({ label }) => label),
    ciDefaultLabels,
  );
  assert.ok(ciMatrix.gates.every((gate) => ["fast-contract", "runtime-core"].includes(gate.tier)));
});

test("acceptance script can write gate results JSON", (t) => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");
  const tempDir = mkdtempSync(join(tmpdir(), "forge-acceptance-results-"));
  t.after(() => rmSync(tempDir, { recursive: true, force: true }));
  const resultsPath = join(tempDir, "gate-results.json");

  execFileSync(
    scriptPath,
    ["--only", "desktop state consistency map status", "--results-json", resultsPath],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  const results = JSON.parse(readFileSync(resultsPath, "utf8"));
  assert.equal(results.schemaVersion, 1);
  assert.equal(results.workingDirectory, root.replace(/\/$/, ""));
  assert.equal(results.status, "passed");
  assert.equal(results.selectedGateCount, 1);
  assert.equal(results.executedGateCount, 1);
  assert.deepEqual(results.gates.map(({ label, status, exitCode }) => ({ label, status, exitCode })), [
    {
      label: "desktop state consistency map status",
      status: "passed",
      exitCode: 0,
    },
  ]);
  assert.deepEqual(
    results.gates.map(({ domain, tier, runtimeCost, manualRequirement, ciDefault }) => ({
      domain,
      tier,
      runtimeCost,
      manualRequirement,
      ciDefault,
    })),
    [
      {
        domain: "usage-context",
        tier: "fast-contract",
        runtimeCost: "short",
        manualRequirement: false,
        ciDefault: true,
      },
    ],
  );
  assert.equal(typeof results.gates[0].durationMs, "number");
});

test("acceptance script reports --grep misses", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--grep", "definitely missing gate"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /No acceptance gates matched --grep: definitely missing gate/);
  assert.match(result.stderr, /Run scripts\/acceptance\.sh --list-json to see valid labels\./);
});

test("acceptance script rejects empty --only values", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--dry-run", "--only", ""], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Missing value for --only/);
});

test("acceptance script rejects empty --grep values", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--dry-run", "--grep", ""], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Missing value for --grep/);
});

test("acceptance script rejects combining --only and --grep", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--only", "desktop production build", "--grep", "provider"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Use only one selector: --only, --grep, or --ci-default\./);
});

test("acceptance script rejects results-json for dry-run output", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--dry-run", "--results-json", "gate-results.json"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Use --results-json only when executing gates\./);
});

test("acceptance script suggests a close gate label for --only misses", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const result = spawnSync(scriptPath, ["--only", "desktop restart blocker documentation status"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /No acceptance gate matched --only: desktop restart blocker documentation status/);
  assert.match(result.stderr, /Did you mean: desktop restart harness blocker documentation status/);
});

test("acceptance script rejects duplicate gate labels", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const tempDir = mkdtempSync(join(tmpdir(), "forge-acceptance-"));
  try {
    const tempScriptPath = join(tempDir, "acceptance.sh");
    const source = readFileSync(scriptPath, "utf8");
    const duplicateLabelSource = source.replace(
      "add_gate 'website production build' 'npm run build:website'",
      "add_gate 'desktop production build' 'npm run build:website'",
    );
    assert.notEqual(duplicateLabelSource, source, "test fixture should inject a duplicate gate label");

    writeFileSync(tempScriptPath, duplicateLabelSource);
    chmodSync(tempScriptPath, 0o755);

    const result = spawnSync(tempScriptPath, ["--dry-run"], {
      cwd: root,
      encoding: "utf8",
    });

    assert.equal(result.status, 1);
    assert.match(result.stderr, /Duplicate acceptance gate label: desktop production build/);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});

test("acceptance script rejects duplicate gate commands", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const tempDir = mkdtempSync(join(tmpdir(), "forge-acceptance-"));
  try {
    const tempScriptPath = join(tempDir, "acceptance.sh");
    const source = readFileSync(scriptPath, "utf8");
    const duplicateCommandSource = source.replace(
      "add_gate 'website production build' 'npm run build:website'",
      "add_gate 'website production build' 'npm run build:desktop'",
    );
    assert.notEqual(duplicateCommandSource, source, "test fixture should inject a duplicate gate command");

    writeFileSync(tempScriptPath, duplicateCommandSource);
    chmodSync(tempScriptPath, 0o755);

    const result = spawnSync(tempScriptPath, ["--dry-run"], {
      cwd: root,
      encoding: "utf8",
    });

    assert.equal(result.status, 1);
    assert.match(result.stderr, /Duplicate acceptance gate command: npm run build:desktop/);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});
