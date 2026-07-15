import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SOURCE_STORES = [
  {
    source: "wiki_memory",
    owner: "desktop_runtime",
    storage: "wiki_memory_store",
    scope: "project_session",
    physicalStore: "legacy_json_or_sqlite_backing",
    archivePolicy: "archive_supported",
    forgetPolicy: "forget_supported",
    recallPolicy: "eligible_when_project_matches_and_active",
    canArchive: true,
    canForget: true,
    canEdit: false,
    migrationAction: "read_only_compare",
  },
  {
    source: "memory_fact",
    owner: "desktop_runtime",
    storage: "memory_fact_store",
    scope: "project_or_profile",
    physicalStore: "legacy_profile_fact_backing",
    archivePolicy: "archive_not_supported",
    forgetPolicy: "forget_supported",
    recallPolicy: "eligible_when_project_or_profile_matches",
    canArchive: false,
    canForget: true,
    canEdit: true,
    migrationAction: "read_only_compare",
  },
  {
    source: "continuity_experience",
    owner: "desktop_runtime",
    storage: "continuity_store",
    scope: "project",
    physicalStore: "legacy_continuity_backing",
    archivePolicy: "archive_supported",
    forgetPolicy: "archive_only",
    recallPolicy: "eligible_when_project_matches_and_active",
    canArchive: true,
    canForget: false,
    canEdit: false,
    migrationAction: "read_only_compare",
  },
  {
    source: "saved_background",
    owner: "desktop_runtime",
    storage: "session_saved_background",
    scope: "session_project",
    physicalStore: "legacy_session_backing",
    archivePolicy: "archive_supported",
    forgetPolicy: "archive_only",
    recallPolicy: "eligible_when_session_or_project_matches",
    canArchive: true,
    canForget: false,
    canEdit: false,
    migrationAction: "design_only",
  },
  {
    source: "project_archive",
    owner: "desktop_runtime",
    storage: "project_archive_projection",
    scope: "project",
    physicalStore: "projection_over_existing_stores",
    archivePolicy: "already_archived_projection",
    forgetPolicy: "source_delegated",
    recallPolicy: "not_injected_directly",
    canArchive: false,
    canForget: false,
    canEdit: false,
    migrationAction: "design_only",
  },
  {
    source: "turn_recall_audit",
    owner: "desktop_runtime",
    storage: "turn_prepared_event",
    scope: "session_turn",
    physicalStore: "runtime_journal_projection",
    archivePolicy: "not_applicable",
    forgetPolicy: "audit_retention_policy",
    recallPolicy: "audit_only",
    canArchive: false,
    canForget: false,
    canEdit: false,
    migrationAction: "do_not_migrate_as_memory_record",
  },
  {
    source: "future_embedding_index",
    owner: "desktop_runtime",
    storage: "not_created",
    scope: "project_or_profile",
    physicalStore: "not_created",
    archivePolicy: "blocked_until_schema_gate",
    forgetPolicy: "blocked_until_schema_gate",
    recallPolicy: "blocked_until_schema_gate",
    canArchive: false,
    canForget: false,
    canEdit: false,
    migrationAction: "future_design_only",
  },
];

const SOURCE_BY_ID = new Map(SOURCE_STORES.map((source) => [source.source, source]));

function defaultFixture() {
  return {
    records: [
      {
        id: "wiki_memory:project-decision",
        source: "wiki_memory",
        source_id: "project-decision",
        status: "accepted",
        visibility: "user_visible",
        title: "Project decision",
        body: "Representative wiki memory body omitted from report.",
      },
      {
        id: "memory_fact:profile-preference",
        source: "memory_fact",
        source_id: "profile-preference",
        status: "accepted",
        visibility: "hidden_context",
        title: "Profile preference",
        text: "Representative profile fact body omitted from report.",
      },
      {
        id: "continuity_experience:last-run",
        source: "continuity_experience",
        source_id: "last-run",
        status: "archived",
        visibility: "audit_only",
        title: "Last run",
        content: "Representative continuity body omitted from report.",
      },
    ],
    expectedRecallIds: ["wiki_memory:project-decision", "memory_fact:profile-preference"],
  };
}

export function parseArgs(argv) {
  const parsed = {
    help: false,
    json: false,
    fixturePath: undefined,
    outPath: undefined,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
    } else if (arg === "--json") {
      parsed.json = true;
    } else if (arg === "--fixture") {
      const value = argv[index + 1];
      if (!value) {
        throw Object.assign(new Error("Missing value for --fixture"), { exitCode: 2 });
      }
      parsed.fixturePath = value;
      index += 1;
    } else if (arg === "--out") {
      const value = argv[index + 1];
      if (!value) {
        throw Object.assign(new Error("Missing value for --out"), { exitCode: 2 });
      }
      parsed.outPath = value;
      index += 1;
    } else {
      throw Object.assign(new Error(`Unknown argument: ${arg}`), { exitCode: 2 });
    }
  }

  return parsed;
}

function loadFixture(fixturePath) {
  if (!fixturePath) {
    return defaultFixture();
  }
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  if (!Array.isArray(fixture.records)) {
    throw Object.assign(new Error("Fixture must include a records array"), { exitCode: 2 });
  }
  return fixture;
}

function normalizeStatus(value) {
  const status = String(value ?? "accepted").toLowerCase();
  if (["active", "current", "accepted"].includes(status)) return "accepted";
  if (["pin", "pinned"].includes(status)) return "pinned";
  if (["archive", "archived"].includes(status)) return "archived";
  if (["forget", "forgotten", "deleted", "removed"].includes(status)) return "forgotten";
  if (["candidate", "pending"].includes(status)) return "candidate";
  return status;
}

function sourceIdFromRecord(record, source, index) {
  if (typeof record.source_id === "string" && record.source_id.length > 0) return record.source_id;
  if (typeof record.sourceId === "string" && record.sourceId.length > 0) return record.sourceId;
  if (typeof record.id === "string" && record.id.startsWith(`${source}:`)) {
    return record.id.slice(source.length + 1);
  }
  if (typeof record.id === "string" && record.id.length > 0) return record.id;
  return `record-${index + 1}`;
}

function availableActionsFor(record, descriptor) {
  const actions = [];
  if (descriptor.canArchive) {
    actions.push(record.status === "archived" ? "restore" : "archive");
  }
  if (descriptor.canForget && record.status !== "forgotten") {
    actions.push("forget");
  }
  if (descriptor.canEdit && record.source === "memory_fact") {
    actions.push("edit");
  }
  return actions;
}

function normalizeRecord(record, index) {
  const source = String(record.source ?? record.kind ?? "unknown").toLowerCase();
  const descriptor =
    SOURCE_BY_ID.get(source) ??
    {
      source,
      owner: "unknown",
      storage: "unknown",
      scope: "unknown",
      archivePolicy: "unknown",
      forgetPolicy: "unknown",
      recallPolicy: "unknown",
      canArchive: false,
      canForget: false,
      canEdit: false,
    };
  const sourceId = sourceIdFromRecord(record, source, index);
  const id = String(record.id ?? `${source}:${sourceId}`);
  const status = normalizeStatus(record.status);
  const activeForRecall = status === "accepted" || status === "pinned";
  const recallEligible =
    Boolean(SOURCE_BY_ID.get(source)) &&
    activeForRecall &&
    record.recallEligible !== false &&
    record.recall_eligible !== false &&
    descriptor.recallPolicy !== "audit_only" &&
    descriptor.recallPolicy !== "not_injected_directly";

  return {
    id,
    source,
    sourceId,
    status,
    visibility: String(record.visibility ?? "user_visible"),
    title: String(record.title ?? record.label ?? `${source}:${sourceId}`),
    owner: descriptor.owner,
    storage: descriptor.storage,
    scope: descriptor.scope,
    archivePolicy: descriptor.archivePolicy,
    forgetPolicy: descriptor.forgetPolicy,
    recallPolicy: descriptor.recallPolicy,
    recallEligible,
    availableActions: availableActionsFor({ source, status }, descriptor),
  };
}

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

function sameSet(left, right) {
  return JSON.stringify(sorted(left)) === JSON.stringify(sorted(right));
}

function buildInvariants(records, fixture) {
  const ids = records.map((record) => record.id);
  const uniqueIds = new Set(ids);
  const identityFailures = records.filter((record) => record.id !== `${record.source}:${record.sourceId}`);
  const unknownSources = records.filter((record) => !SOURCE_BY_ID.has(record.source));
  const leakedBodyFields = records.filter((record) =>
    ["body", "text", "content", "rawBody", "raw_body", "memory_body"].some((field) =>
      Object.hasOwn(record, field),
    ),
  );
  const actualRecallIds = records.filter((record) => record.recallEligible).map((record) => record.id);
  const expectedRecallIds = Array.isArray(fixture.expectedRecallIds)
    ? fixture.expectedRecallIds.map(String)
    : actualRecallIds;

  return [
    {
      id: "record_identity_stable",
      label: "Record identity remains source-prefixed and unique",
      passed: identityFailures.length === 0 && uniqueIds.size === ids.length,
      details:
        identityFailures.length === 0 && uniqueIds.size === ids.length
          ? [`${ids.length} record ids preserved`]
          : [
              `identity failures: ${identityFailures.map((record) => record.id).join(", ") || "none"}`,
              `duplicate ids: ${ids.length - uniqueIds.size}`,
            ],
    },
    {
      id: "archive_forget_semantics_stable",
      label: "Archive and forget semantics remain source-owned",
      passed: unknownSources.length === 0,
      details:
        unknownSources.length === 0
          ? ["all records map to a known memory authority source"]
          : [`unknown sources: ${unknownSources.map((record) => record.source).join(", ")}`],
    },
    {
      id: "recall_results_stable",
      label: "Recall decisions match the expected dry-run fixture",
      passed: sameSet(actualRecallIds, expectedRecallIds),
      details: [
        `actual=${sorted(actualRecallIds).join(",") || "none"}`,
        `expected=${sorted(expectedRecallIds).join(",") || "none"}`,
      ],
    },
    {
      id: "hidden_bodies_not_exported",
      label: "Hidden memory bodies are not exported in the migration report",
      passed: leakedBodyFields.length === 0,
      details:
        leakedBodyFields.length === 0
          ? ["report records contain metadata only"]
          : [`leaked records: ${leakedBodyFields.map((record) => record.id).join(", ")}`],
    },
    {
      id: "legacy_store_retained",
      label: "Legacy stores remain the runtime source of truth",
      passed: true,
      details: ["dry-run does not create, mutate, or drop any physical memory store"],
    },
    {
      id: "rollback_plan_present",
      label: "Rollback plan is attached before any physical migration",
      passed: true,
      details: ["rollback plan keeps legacy stores and dual-read verification available"],
    },
  ];
}

function rollbackPlan() {
  return [
    {
      step: 1,
      action: "snapshot_legacy_stores",
      detail: "Take an operator-visible snapshot of wiki memory, memory facts, continuity, saved background, and project archive projections before enabling any unified physical store.",
    },
    {
      step: 2,
      action: "dual_read_compare_only",
      detail: "Run unified SQLite as a shadow read model and compare record ids, archive/forget semantics, recall eligibility, and hidden-body redaction against the legacy stores.",
    },
    {
      step: 3,
      action: "disable_unified_store_flag",
      detail: "If any invariant fails, turn off the unified physical store capability flag and keep desktop runtime reads on the legacy stores.",
    },
    {
      step: 4,
      action: "restore_legacy_snapshot",
      detail: "When a migration write has been attempted in a future phase, restore the legacy snapshot before replaying runtime journals.",
    },
    {
      step: 5,
      action: "rerun_dry_run_and_acceptance",
      detail: "Re-run this dry-run report and the memory acceptance gates before retrying migration or claiming readiness.",
    },
  ];
}

export function buildMemoryMigrationDryRunReport({ fixture, reportWritePerformed = false } = {}) {
  const loadedFixture = fixture ?? defaultFixture();
  const records = loadedFixture.records.map(normalizeRecord);
  const invariants = buildInvariants(records, loadedFixture);
  const counts = {
    total: records.length,
    archived: records.filter((record) => record.status === "archived").length,
    forgotten: records.filter((record) => record.status === "forgotten").length,
    recallEligible: records.filter((record) => record.recallEligible).length,
  };

  return {
    schemaVersion: 1,
    mode: "dry-run",
    targetStore: "unified_sqlite",
    writesPerformed: false,
    reportWritePerformed,
    physicalStoreMigrationStarted: false,
    readyForPhysicalMigration: false,
    sourceStores: SOURCE_STORES,
    records,
    invariants,
    rollbackPlan: rollbackPlan(),
    summary: {
      recordCounts: counts,
      invariantStatus: invariants.every((invariant) => invariant.passed) ? "passed" : "failed",
      readyForPhysicalMigration: false,
      blockers: [
        "operator_approval_required",
        "live_store_migration_not_started",
        "dual_read_compare_not_yet_run_against_live_data",
      ],
      nextGate: "dry_run_against_live_snapshot_before_unified_sqlite_apply",
    },
  };
}

function renderText(report) {
  const lines = [
    "Forge memory physical migration dry-run report",
    `mode: ${report.mode}`,
    `target store: ${report.targetStore}`,
    `writes performed: ${report.writesPerformed}`,
    `physical migration started: ${report.physicalStoreMigrationStarted}`,
    `ready for physical migration: ${report.readyForPhysicalMigration}`,
    `records: ${report.summary.recordCounts.total}`,
    "invariants:",
  ];
  for (const invariant of report.invariants) {
    lines.push(`- ${invariant.id}: ${invariant.passed ? "passed" : "failed"}`);
  }
  lines.push("rollback plan:");
  for (const step of report.rollbackPlan) {
    lines.push(`- ${step.step}. ${step.action}: ${step.detail}`);
  }
  return `${lines.join("\n")}\n`;
}

function printHelp() {
  return `Usage: node scripts/memory-migration-dry-run.mjs [--json] [--fixture <path>] [--out <path>]

Builds a read-only memory physical store migration design report.
It does not create, mutate, or migrate any memory store.
`;
}

export function main(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  if (args.help) {
    process.stdout.write(printHelp());
    return 0;
  }

  const fixture = loadFixture(args.fixturePath);
  const report = buildMemoryMigrationDryRunReport({ fixture, reportWritePerformed: Boolean(args.outPath) });
  if (args.outPath) {
    mkdirSync(dirname(args.outPath), { recursive: true });
    writeFileSync(args.outPath, `${JSON.stringify(report, null, 2)}\n`);
  }

  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    process.stdout.write(renderText(report));
  }
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.exitCode = main();
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = error.exitCode ?? 1;
  }
}
