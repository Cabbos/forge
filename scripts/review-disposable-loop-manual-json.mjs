#!/usr/bin/env node
import { readFileSync } from "node:fs";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { createDisposableLoopManualTemplate } from "./create-disposable-loop-manual-json.mjs";

export function reviewDisposableLoopManualJson({
  row = "1",
  manualValues = null,
  requireComplete = false,
} = {}) {
  const normalizedRow = String(row);
  const expected = createDisposableLoopManualTemplate({ row: normalizedRow });
  const values = manualValues && typeof manualValues === "object" && !Array.isArray(manualValues)
    ? manualValues
    : createDisposableLoopManualTemplate({ row: normalizedRow, includePrompt: false });
  const issues = [];
  const warnings = [];
  const expectedLabels = Object.keys(expected);

  for (const label of expectedLabels) {
    if (!Object.hasOwn(values, label)) {
      issues.push({ code: "field_missing", field: label, message: `Missing manual evidence field: ${label}.` });
      continue;
    }
    const value = String(values[label] ?? "").trim();
    if (!value) {
      issues.push({ code: "field_empty", field: label, message: `Manual evidence field is empty: ${label}.` });
      continue;
    }
    if (looksPlaceholder(value)) {
      issues.push({ code: "field_placeholder", field: label, message: `Manual evidence field still looks like a placeholder: ${label}.` });
    }
  }

  if (String(values["Forge prompt"] ?? "").trim() && String(values["Forge prompt"]).trim() !== expected["Forge prompt"].trim()) {
    issues.push({
      code: "prompt_mismatch",
      field: "Forge prompt",
      message: "Forge prompt does not match the exact prompt for this row.",
    });
  }

  for (const label of Object.keys(values)) {
    if (!expectedLabels.includes(label)) {
      warnings.push({ code: "unexpected_field", field: label, message: `Unexpected manual evidence field will be ignored: ${label}.` });
    }
  }

  const pass = issues.length === 0;
  return {
    status: pass ? "complete" : requireComplete ? "incomplete" : "pending_manual_evidence",
    pass,
    row: normalizedRow,
    requireComplete,
    expectedFields: expectedLabels,
    issues,
    warnings,
    summary: {
      missingFields: issues.filter((issue) => issue.code === "field_missing").map((issue) => issue.field),
      emptyFields: issues.filter((issue) => issue.code === "field_empty").map((issue) => issue.field),
      placeholderFields: issues.filter((issue) => issue.code === "field_placeholder").map((issue) => issue.field),
      promptMatches: !issues.some((issue) => issue.code === "prompt_mismatch"),
    },
  };
}

function looksPlaceholder(value) {
  return /^(todo|tbd|n\/a|na|none|\(none\)|待补|待填写|TODO)$/i.test(value.trim());
}

function printHelp() {
  console.log(`Usage: node scripts/review-disposable-loop-manual-json.mjs [--json] [--row <all|1|2|3>] [--manual-json <path>] [--require-complete]

Reviews Phase 8 disposable loop manual evidence JSON before strict validation/archive.

Options:
  --json                Print machine-readable review.
  --row VALUE           Row scope: all, 1, 2, or 3. Defaults to 1.
  --manual-json PATH    JSON object keyed by manual evidence field label.
  --require-complete    Exit non-zero unless all manual fields are present, non-placeholder, and prompt-matched.
  -h, --help            Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    row: "1",
    manualJson: null,
    requireComplete: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--require-complete") {
      options.requireComplete = true;
    } else if (arg === "--row") {
      const value = argv[index + 1];
      if (!["all", "1", "2", "3"].includes(value)) throw new Error("--row must be one of: all, 1, 2, 3");
      options.row = value;
      index += 1;
    } else if (arg === "--manual-json") {
      const value = argv[index + 1];
      if (!value) throw new Error("--manual-json requires a path");
      options.manualJson = value;
      index += 1;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function readManualValues(path) {
  if (!path) return null;
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("--manual-json must contain a JSON object");
  }
  return parsed;
}

function printHuman(result) {
  console.log("Disposable loop manual evidence review");
  console.log(`Status: ${result.status}`);
  console.log(`Row: ${result.row}`);
  console.log(`Pass: ${result.pass ? "yes" : "no"}`);
  if (result.issues.length > 0) {
    console.log("Issues:");
    for (const issue of result.issues) {
      console.log(`- ${issue.code}: ${issue.message}`);
    }
  }
  if (result.warnings.length > 0) {
    console.log("Warnings:");
    for (const warning of result.warnings) {
      console.log(`- ${warning.code}: ${warning.message}`);
    }
  }
}

function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
    options.manualValues = readManualValues(options.manualJson);
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  if (options.help) {
    printHelp();
    return 0;
  }

  const result = reviewDisposableLoopManualJson(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return options.requireComplete && !result.pass ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
