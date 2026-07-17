#!/usr/bin/env node
/**
 * StreamEvent Protocol Cross-Check
 *
 * Layer 1 (existing): ensures every `#[serde(rename = "...")]` variant in the
 * Rust StreamEvent enum is explicitly handled by the frontend event dispatcher
 * (or the blocks.ts `eventToBlock` fallback), and present in the
 * src/lib/protocol.ts StreamEvent union.
 *
 * Layer 2 (structural): parses each Rust variant's fields and each TypeScript
 * union member's properties and diffs them field-by-field, so adding a field
 * on one side only can no longer pass silently:
 *   - field name present on one side only        -> FAIL
 *   - Rust may omit a key TS assumes is present  -> FAIL
 *     (`skip_serializing_if` vs non-optional TS property)
 *   - Rust may send null TS declares non-nullable -> FAIL
 *     (`Option<T>` serialized as null vs TS type without `| null`)
 *   - TS looser than Rust (optional/`| null` where Rust guarantees a value)
 *                                                  -> warning only (safe direction)
 *
 * Usage: node scripts/check-protocol-sync.mjs
 * Exit 0 if all checks pass, 1 otherwise.
 */

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

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
// ALLOWED_FIELD_MISMATCH
// ---------------------------------------------------------------------------
// Per-variant field-level exceptions for Layer 2. Each entry MUST include a
// comment justifying why the asymmetry is intentional. Never add entries to
// hide a real mismatch.
// Shape: { "<event_type>": ["field_name", ...] }
const ALLOWED_FIELD_MISMATCH = {
  // Rust marks block_id with `#[serde(default = ...)]` so historical events
  // serialized before the field existed still replay; the frontend may read
  // those old persisted blocks from IndexedDB, so TS keeps it optional on
  // purpose. New emissions always carry a non-null block_id.
  provider_usage: ["block_id"],
};

// ---------------------------------------------------------------------------
// Shared parsing helpers (exported for tests)
// ---------------------------------------------------------------------------

function stripLineComment(line) {
  return line.replace(/\/\/.*$/, "");
}

function braceDelta(s) {
  return (s.match(/\{/g) || []).length - (s.match(/\}/g) || []).length;
}

// Split on commas that are not inside `<>`, `()`, or `[]` so generic types
// like `HashMap<String, Vec<String>>` stay in one piece.
function splitTopLevel(s) {
  const out = [];
  let depth = 0;
  let cur = "";
  for (const ch of s) {
    if (ch === "<" || ch === "(" || ch === "[") depth += 1;
    if (ch === ">" || ch === ")" || ch === "]") depth -= 1;
    if (ch === "," && depth === 0) {
      out.push(cur);
      cur = "";
    } else {
      cur += ch;
    }
  }
  if (cur.trim()) out.push(cur);
  return out;
}

function parseRustField(segment, attrs) {
  const m = segment.trim().match(/^([a-z_][a-z0-9_]*)\s*:\s*(.+?)(,)?$/);
  if (!m) return null;
  const rename = (attrs || "").match(/rename\s*=\s*"([^"]+)"/);
  const type = m[2].trim();
  return {
    name: rename ? rename[1] : m[1],
    // Serialized JSON omits the key only when skip_serializing_if applies.
    mayBeAbsent: (attrs || "").includes("skip_serializing_if"),
    // Option<T> without skip_serializing_if serializes as an explicit null.
    nullable: /^Option</.test(type),
  };
}

/**
 * Parse `enum StreamEvent` from the Rust source.
 * Returns a Map<eventType, Array<{name, mayBeAbsent, nullable}>>.
 * Handles both multi-line rustfmt variants and single-line variants like
 * `SessionStopped { session_id: String, reason: String },`.
 */
export function parseRustStreamEvents(rustSource) {
  const enumMatch = rustSource.match(/pub enum StreamEvent \{([\s\S]*?)\n\}/);
  if (!enumMatch) {
    throw new Error("could not locate `enum StreamEvent { ... }` block in Rust source");
  }
  const variants = new Map();
  let cur = null;
  let pendingRename = null;
  let depth = 0;

  const finalize = () => {
    if (cur) {
      variants.set(cur.tag, cur.fields);
      cur = null;
    }
  };

  for (const raw of enumMatch[1].split("\n")) {
    const line = stripLineComment(raw).trim();

    if (cur === null) {
      const rn = line.match(/#\[serde\(rename\s*=\s*"([^"]+)"\)\]/);
      if (rn) {
        pendingRename = rn[1];
        continue;
      }
      if (pendingRename) {
        const vm = line.match(/^([A-Z][A-Za-z0-9_]*)\s*\{/);
        if (vm) {
          cur = { tag: pendingRename, fields: [], attrs: "" };
          pendingRename = null;
          depth = braceDelta(line);
          if (depth === 0) {
            // Single-line variant: fields live between the braces on this line.
            const inner = line.slice(line.indexOf("{") + 1, line.lastIndexOf("}"));
            for (const seg of splitTopLevel(inner)) {
              const f = parseRustField(seg, "");
              if (f) cur.fields.push(f);
            }
            finalize();
          }
          continue;
        }
      }
      continue;
    }

    // Inside a multi-line variant.
    if (line.startsWith("#[")) {
      cur.attrs += ` ${line}`;
      depth += braceDelta(line);
      continue;
    }
    const field = parseRustField(line, cur.attrs);
    if (field) {
      cur.fields.push(field);
      cur.attrs = "";
    }
    depth += braceDelta(line);
    if (depth <= 0) finalize();
  }

  if (variants.size === 0) {
    throw new Error("could not extract any StreamEvent variants from Rust enum");
  }
  return variants;
}

/**
 * Parse the `export type StreamEvent = ...` union from the TypeScript source.
 * Returns a Map<eventType, Array<{name, optional, nullable}>>.
 */
export function parseTsStreamEvents(tsSource) {
  const startIdx = tsSource.indexOf("export type StreamEvent");
  if (startIdx === -1) {
    throw new Error("could not locate `export type StreamEvent` in TypeScript source");
  }
  // The union ends at the multi-line member terminator `\n    };`.
  const endIdx = tsSource.indexOf("\n    };", startIdx);
  if (endIdx === -1) {
    throw new Error("could not locate the end of the StreamEvent union in TypeScript source");
  }
  const body = tsSource.slice(startIdx, endIdx);
  const variants = new Map();

  for (const member of body.split(/\n\s*\|\s*/)) {
    const tagM = member.match(/event_type:\s*"([^"]+)"/);
    if (!tagM) continue;
    let inner = member.slice(member.indexOf("{") + 1);
    const lastBrace = inner.lastIndexOf("}");
    if (lastBrace !== -1) inner = inner.slice(0, lastBrace);
    const fields = [];
    for (let seg of inner.split(";")) {
      seg = stripLineComment(seg).trim();
      if (!seg || seg.startsWith("event_type")) continue;
      const fm = seg.match(/^([a-zA-Z_][a-zA-Z0-9_]*)(\?)?\s*:\s*([\s\S]+)$/);
      if (fm) {
        fields.push({
          name: fm[1],
          optional: Boolean(fm[2]),
          nullable: /\|\s*null/.test(fm[3]),
        });
      }
    }
    variants.set(tagM[1], fields);
  }

  if (variants.size === 0) {
    throw new Error("could not extract any StreamEvent members from TypeScript union");
  }
  return variants;
}

/**
 * Diff Rust variants against TS union members field-by-field.
 * Returns { errors: string[], warnings: string[] }.
 */
export function diffProtocolFields(rustVariants, tsVariants, allowed = ALLOWED_FIELD_MISMATCH) {
  const errors = [];
  const warnings = [];

  const isAllowed = (tag, field) => Array.isArray(allowed[tag]) && allowed[tag].includes(field);

  for (const [tag, rustFields] of rustVariants) {
    const tsFields = tsVariants.get(tag);
    if (!tsFields) continue; // missing-variant case is reported by Layer 1

    const tsByName = new Map(tsFields.map((f) => [f.name, f]));
    const rustByName = new Map(rustFields.map((f) => [f.name, f]));

    for (const rf of rustFields) {
      if (isAllowed(tag, rf.name)) continue;
      const tf = tsByName.get(rf.name);
      if (!tf) {
        errors.push(`${tag}: Rust field "${rf.name}" is missing from src/lib/protocol.ts`);
        continue;
      }
      if (rf.mayBeAbsent && !tf.optional) {
        errors.push(
          `${tag}.${rf.name}: Rust may omit this key (skip_serializing_if) but TS declares it always-present`,
        );
      }
      if (rf.nullable && !tf.nullable && !tf.optional) {
        errors.push(
          `${tag}.${rf.name}: Rust may serialize null (Option<T>) but TS type has no \`| null\``,
        );
      }
      if (!rf.mayBeAbsent && !rf.nullable && (tf.optional || tf.nullable)) {
        warnings.push(
          `${tag}.${rf.name}: TS is looser than Rust (Rust always sends a value; TS marks it optional/nullable)`,
        );
      }
    }

    for (const tf of tsFields) {
      if (isAllowed(tag, tf.name)) continue;
      if (!rustByName.has(tf.name)) {
        errors.push(`${tag}: TS field "${tf.name}" has no counterpart in the Rust StreamEvent enum`);
      }
    }
  }

  return { errors, warnings };
}

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

function main() {
  // -------------------------------------------------------------------------
  // Layer 1a: extract Rust StreamEvent types
  // -------------------------------------------------------------------------

  const RUST_PATH = join(ROOT, "src-tauri/src/protocol/events.rs");
  const rustSource = readFileOrExit(RUST_PATH, "Rust events.rs");

  let rustVariants;
  try {
    rustVariants = parseRustStreamEvents(rustSource);
  } catch (err) {
    console.error(`FAIL: ${err.message}`);
    process.exit(1);
  }
  const rustEventTypes = new Set(rustVariants.keys());

  // -------------------------------------------------------------------------
  // Layer 1b: extract handled event types from the TypeScript dispatcher
  // -------------------------------------------------------------------------

  const TS_PATH = join(ROOT, "src/store/event-dispatch.ts");
  const tsSource = readFileOrExit(TS_PATH, "TypeScript event-dispatch.ts");

  const BLOCKS_PATH = join(ROOT, "src/store/blocks.ts");
  const blocksSource = readFileOrExit(BLOCKS_PATH, "TypeScript blocks.ts");

  const PROTOCOL_PATH = join(ROOT, "src/lib/protocol.ts");
  const protocolSource = readFileOrExit(PROTOCOL_PATH, "TypeScript lib/protocol.ts");

  const handledTypes = new Set();

  // 1. Direct `event_type === "..."` comparisons in the dispatcher
  const eqRegex = /\bevent_type\s*===?\s*"([^"]+)"/g;
  let match;
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
  //      when event-dispatch.ts has no explicit branch for it, as long as
  //      eventToBlock knows about it. Scanning blocks.ts ensures we do not flag
  //      these fallback-handled events as unhandled.
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

  // -------------------------------------------------------------------------
  // Layer 1c: cross-check against src/lib/protocol.ts StreamEvent union
  // -------------------------------------------------------------------------

  let tsVariants;
  try {
    tsVariants = parseTsStreamEvents(protocolSource);
  } catch (err) {
    console.error(`FAIL: ${err.message}`);
    process.exit(1);
  }

  const missingFromProtocol = [];
  for (const t of rustEventTypes) {
    if (!tsVariants.has(t)) {
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

  // -------------------------------------------------------------------------
  // Layer 1d: compare Rust types against handled types
  // -------------------------------------------------------------------------

  const missing = [];
  for (const t of rustEventTypes) {
    if (!handledTypes.has(t) && !ALLOWED_UNHANDLED.has(t)) {
      missing.push(t);
    }
  }

  if (missing.length > 0) {
    console.log("FAIL: Rust emits event types that the frontend does not handle:");
    for (const t of missing.sort()) {
      console.log(`  - ${t}`);
    }
    process.exit(1);
  }

  // -------------------------------------------------------------------------
  // Layer 2: field-level structural diff
  // -------------------------------------------------------------------------

  const { errors, warnings } = diffProtocolFields(rustVariants, tsVariants);

  for (const w of warnings) {
    console.log(`WARN: ${w}`);
  }

  if (errors.length > 0) {
    console.log("FAIL: StreamEvent field-level mismatch between Rust and TypeScript:");
    for (const e of errors.sort()) {
      console.log(`  - ${e}`);
    }
    process.exit(1);
  }

  const variantWord = `${rustEventTypes.size} Rust StreamEvent types`;
  const warnSuffix = warnings.length > 0 ? ` (${warnings.length} loose-direction warnings)` : "";
  console.log(`OK: all ${variantWord} are handled and field-aligned with protocol.ts${warnSuffix}`);
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
if (isMain) {
  main();
}
