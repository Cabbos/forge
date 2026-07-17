import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  parseRustStreamEvents,
  parseTsStreamEvents,
  diffProtocolFields,
} from "./check-protocol-sync.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");

test("parseRustStreamEvents handles multi-line and single-line variants", () => {
  const rust = `
pub enum StreamEvent {
    #[serde(rename = "text_chunk")]
    TextChunk {
        session_id: String,
        block_id: String,
        content: String,
    },
    #[serde(rename = "session_stopped")]
    SessionStopped { session_id: String, reason: String },
    #[serde(rename = "file_io")]
    FileIo {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        tags: HashMap<String, Vec<String>>,
    },
}
`;
  const variants = parseRustStreamEvents(rust);
  assert.equal(variants.size, 3);
  assert.deepEqual(
    variants.get("text_chunk").map((f) => f.name),
    ["session_id", "block_id", "content"],
  );
  assert.deepEqual(
    variants.get("session_stopped").map((f) => f.name),
    ["session_id", "reason"],
  );
  const fileIo = variants.get("file_io");
  assert.deepEqual(
    fileIo.map((f) => f.name),
    ["session_id", "source", "tags"],
  );
  assert.equal(fileIo[1].mayBeAbsent, true);
  assert.equal(fileIo[1].nullable, true);
  assert.equal(fileIo[2].nullable, false, "HashMap field must not read as Option");
});

test("parseTsStreamEvents handles single-line and multi-line members", () => {
  const ts = `
export type StreamEvent =
  | { event_type: "text_chunk"; session_id: string; block_id: string; content: string }
  | {
      event_type: "file_io";
      session_id: string;
      source?: string | null;
    };
`;
  const variants = parseTsStreamEvents(ts);
  assert.equal(variants.size, 2);
  assert.deepEqual(
    variants.get("text_chunk").map((f) => f.name),
    ["session_id", "block_id", "content"],
  );
  const fileIo = variants.get("file_io");
  assert.equal(fileIo[1].name, "source");
  assert.equal(fileIo[1].optional, true);
  assert.equal(fileIo[1].nullable, true);
});

test("diffProtocolFields flags missing, extra, and unsafe optionality", () => {
  const rust = new Map([
    [
      "confirm_ask",
      [
        { name: "session_id", mayBeAbsent: false, nullable: false },
        { name: "boundary", mayBeAbsent: true, nullable: true },
        { name: "note", mayBeAbsent: false, nullable: true },
      ],
    ],
  ]);
  const ts = new Map([
    [
      "confirm_ask",
      [
        { name: "session_id", optional: false, nullable: false },
        { name: "boundary", optional: false, nullable: false },
        { name: "note", optional: false, nullable: false },
        { name: "extra", optional: true, nullable: false },
      ],
    ],
  ]);
  const { errors, warnings } = diffProtocolFields(rust, ts, {});
  assert.ok(errors.some((e) => e.includes("boundary") && e.includes("skip_serializing_if")));
  assert.ok(errors.some((e) => e.includes("note") && e.includes("| null")));
  assert.ok(errors.some((e) => e.includes('TS field "extra"')));
  assert.equal(warnings.length, 0);
});

test("diffProtocolFields treats looser TS as warning, not failure", () => {
  const rust = new Map([
    ["usage", [{ name: "input_tokens", mayBeAbsent: false, nullable: false }]],
  ]);
  const ts = new Map([
    ["usage", [{ name: "input_tokens", optional: true, nullable: true }]],
  ]);
  const { errors, warnings } = diffProtocolFields(rust, ts, {});
  assert.equal(errors.length, 0);
  assert.equal(warnings.length, 1);
  assert.ok(warnings[0].includes("usage.input_tokens"));
});

test("diffProtocolFields honors the justified allowlist", () => {
  const rust = new Map([
    ["usage", [{ name: "input_tokens", mayBeAbsent: false, nullable: false }]],
  ]);
  const ts = new Map([
    ["usage", [{ name: "input_tokens", optional: true, nullable: true }]],
  ]);
  const { errors, warnings } = diffProtocolFields(rust, ts, { usage: ["input_tokens"] });
  assert.equal(errors.length, 0);
  assert.equal(warnings.length, 0);
});

test("real protocol files stay field-aligned", () => {
  const rust = readFileSync(join(ROOT, "src-tauri/src/protocol/events.rs"), "utf-8");
  const ts = readFileSync(join(ROOT, "src/lib/protocol.ts"), "utf-8");
  const rustVariants = parseRustStreamEvents(rust);
  const tsVariants = parseTsStreamEvents(ts);
  assert.equal(rustVariants.size, tsVariants.size, "variant count drift");
  const { errors } = diffProtocolFields(rustVariants, tsVariants);
  assert.deepEqual(errors, []);
});
