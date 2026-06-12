#!/usr/bin/env node
/**
 * StreamEvent Protocol Cross-Check
 *
 * Ensures every `#[serde(rename = "...")]` variant in the Rust StreamEvent
 * enum is explicitly handled by the frontend event dispatcher.
 *
 * Usage: node scripts/check-protocol-sync.mjs
 * Exit 0 if all Rust event types are handled, 1 otherwise.
 */

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");

// ---------------------------------------------------------------------------
// ALLOWED_UNHANDLED
// ---------------------------------------------------------------------------
// Each entry MUST include a comment justifying why the frontend intentionally
// no-ops this event type. Never add entries to hide a real mismatch.
const ALLOWED_UNHANDLED = new Set([
  // "example_type", // justified: frontend receives but silently drops because ...
]);

// ---------------------------------------------------------------------------
// Helper: read file with existence check
// ---------------------------------------------------------------------------

function readFileOrExit(path, label) {
  try {
    return readFileSync(path, "utf-8");
  } catch (err) {
    console.error(`FAIL: could not read ${label} at ${path}`);
    if (err.code === "ENOENT") {
      console.error(`  Reason: file does not exist`);
    } else {
      console.error(`  Reason: ${err.message}`);
    }
    process.exit(1);
  }
}

// ---------------------------------------------------------------------------
// Extract Rust StreamEvent types
// ---------------------------------------------------------------------------

const RUST_PATH = join(ROOT, "src-tauri/src/protocol/events.rs");
const rustSource = readFileOrExit(RUST_PATH, "Rust events.rs");

const rustEventTypes = new Set();

// Scope extraction to the `enum StreamEvent { ... }` block only.
// This prevents collecting `#[serde(rename = "...")]` values from other
// structs or enums that may also appear in the file.
const enumMatch = rustSource.match(/pub enum StreamEvent \{([\s\S]*?)\n\}/);
if (!enumMatch) {
  console.error("FAIL: could not locate `enum StreamEvent { ... }` block in Rust source");
  process.exit(1);
}

const enumBody = enumMatch[1];
const renameRegex = /#\[serde\(rename\s*=\s*"([^"]+)"\)\]/g;
let match;
while ((match = renameRegex.exec(enumBody)) !== null) {
  rustEventTypes.add(match[1]);
}

if (rustEventTypes.size === 0) {
  console.error("FAIL: could not extract any StreamEvent types from Rust enum");
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Extract handled event types from the TypeScript dispatcher
// ---------------------------------------------------------------------------

const TS_PATH = join(ROOT, "src/store/event-dispatch.ts");
const tsSource = readFileOrExit(TS_PATH, "TypeScript event-dispatch.ts");

const BLOCKS_PATH = join(ROOT, "src/store/blocks.ts");
const blocksSource = readFileOrExit(BLOCKS_PATH, "TypeScript blocks.ts");

const PROTOCOL_PATH = join(ROOT, "src/lib/protocol.ts");
const protocolSource = readFileOrExit(PROTOCOL_PATH, "TypeScript lib/protocol.ts");

const handledTypes = new Set();

// 1. Direct `event_type === "..."` comparisons in the dispatcher
const eqRegex = /\bevent_type\s*===?\s*"([^"]+)"/g;
while ((match = eqRegex.exec(tsSource)) !== null) {
  handledTypes.add(match[1]);
}

// 2. String literals inside known arrays in the dispatcher
const knownArrayNames = ["CHUNK_TYPES", "END_TYPES"];
for (const arrayName of knownArrayNames) {
  const arrayRegex = new RegExp(`const\\s+${arrayName}\\s*=\\s*\\[([^\\]]*)\\]`);
  const arrayMatch = tsSource.match(arrayRegex);
  if (arrayMatch) {
    const stringRegex = /"([^"]+)"/g;
    let stringMatch;
    while ((stringMatch = stringRegex.exec(arrayMatch[1])) !== null) {
      handledTypes.add(stringMatch[1]);
    }
  }
}

// 3. Event types handled by `eventToBlock` in blocks.ts
//
//    Why we scan blocks.ts as well as event-dispatch.ts:
//    The frontend has two paths for handling StreamEvents:
//    - event-dispatch.ts (the live dispatcher) handles most events directly.
//    - blocks.ts::eventToBlock() is the fallback that converts events to BlockState
//      objects. Events that the dispatcher does not explicitly match fall through
//      to `eventToBlock(event)`. If `eventToBlock` returns null, the event is
//      silently dropped. This means an event CAN be "handled" (not missing) even
//    when event-dispatch.ts has no explicit branch for it, as long as
//    eventToBlock knows about it. Scanning blocks.ts ensures we do not flag
//    these fallback-handled events as unhandled.
//
//    This is intentional defensive coverage beyond the spec — it accounts for
//    the actual frontend architecture rather than an idealized single-dispatcher
//    model.
const eventToBlockMatch = blocksSource.match(/export function eventToBlock\([\s\S]*?\n\}/);
if (eventToBlockMatch) {
  const eventToBlockBody = eventToBlockMatch[0];
  const caseRegex = /case\s+"([^"]+)":/g;
  while ((match = caseRegex.exec(eventToBlockBody)) !== null) {
    handledTypes.add(match[1]);
  }
}

// ---------------------------------------------------------------------------
// Cross-check against src/lib/protocol.ts StreamEvent union
// ---------------------------------------------------------------------------

const protocolDiscriminants = new Set();
const protocolEventRegex = /event_type:\s*"([^"]+)"/g;
while ((match = protocolEventRegex.exec(protocolSource)) !== null) {
  protocolDiscriminants.add(match[1]);
}

const missingFromProtocol = [];
for (const t of rustEventTypes) {
  if (!protocolDiscriminants.has(t)) {
    missingFromProtocol.push(t);
  }
}

if (missingFromProtocol.length > 0) {
  console.error("FAIL: Rust emits event types that are missing from src/lib/protocol.ts StreamEvent union:");
  for (const t of missingFromProtocol.sort()) {
    console.error(`  - ${t}`);
  }
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Compare Rust types against handled types
// ---------------------------------------------------------------------------

const missing = [];
for (const t of rustEventTypes) {
  if (!handledTypes.has(t) && !ALLOWED_UNHANDLED.has(t)) {
    missing.push(t);
  }
}

if (missing.length === 0) {
  console.log(`OK: all ${rustEventTypes.size} Rust StreamEvent types are handled`);
  process.exit(0);
} else {
  console.log("FAIL: Rust emits event types that the frontend does not handle:");
  for (const t of missing.sort()) {
    console.log(`  - ${t}`);
  }
  process.exit(1);
}
