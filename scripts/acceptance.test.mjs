import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
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

test("acceptance script dry-run lists the final product gates", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(scriptPath, ["--dry-run"], {
    cwd: root,
    encoding: "utf8",
  });

  const dryRunEntries = parseDryRunEntries(output);
  const expectedEntries = [
    { label: "desktop production build", command: "npm run build:desktop" },
    { label: "website production build", command: "npm run build:website" },
    { label: "eval runner test suite", command: "npm run test:eval" },
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
      label: "gateway loop runner status smoke",
      command:
        "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_runtime_status_returns_queue_and_run_summary --lib",
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
      label: "A2A child runtime file IO bridge",
      command: "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib",
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
      label: "confirmation response replay contract tests",
      command:
        'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib && npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "confirm response replay|startup transcript hydration"',
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
    "confirmation response replay contract tests",
    "desktop UI evidence observer preflight",
    "desktop UI evidence doctor",
    "desktop UI evidence recovery checks",
    "manual desktop restart smoke protocol",
    "manual stability regression batch",
    "manual disposable edit/build loop protocol",
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
