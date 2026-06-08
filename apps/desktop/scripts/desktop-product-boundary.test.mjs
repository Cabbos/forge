import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;
const srcDir = join(root, "src");

/**
 * Recursively list all .ts and .tsx files under a directory.
 */
function listTsFiles(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      listTsFiles(full, files);
    } else if (st.isFile() && (entry.endsWith(".ts") || entry.endsWith(".tsx"))) {
      files.push(full);
    }
  }
  return files;
}

/**
 * Parse imports from a TypeScript/TSX file.
 * Returns array of { source, raw } where source is the import path string.
 */
function parseImports(content) {
  const imports = [];
  // Match both:
  //   import { x } from "path"
  //   import type { x } from "path"
  //   import x from "path"
  //   import * as x from "path"
  const regex = /import\s+(?:type\s+)?(?:[^'"]+?)\s+from\s+['"]([^'"]+)['"];?/g;
  let match;
  while ((match = regex.exec(content)) !== null) {
    imports.push({ source: match[1], raw: match[0] });
  }
  return imports;
}

/**
 * Check whether an import source starts with any of the given prefixes.
 */
function matchesAnyPrefix(source, prefixes) {
  return prefixes.some((p) => source.startsWith(p));
}

/**
 * Build the relative path from srcDir for a full file path.
 */
function relativeSrc(filePath) {
  return filePath.replace(srcDir + "/", "");
}

// ── Rules ────────────────────────────────────────────────────────────

/**
 * Each rule defines a boundary:
 * - sourceDir:  directory whose files are checked
 * - forbidden:  import path prefixes that are forbidden
 * - allowList:  specific { file, importPrefix, reason } entries that are
 *               permitted despite matching a forbidden prefix
 */
const RULES = [
  {
    name: "primitives must not import product surfaces",
    sourceDir: "src/components/primitives",
    forbidden: [
      "@/components/layout",
      "@/components/session",
      "@/components/chat",
      "@/components/messages",
      "@/components/settings",
      "@/components/context",
    ],
    allowList: [],
  },
  {
    name: "messages must not import session or settings",
    sourceDir: "src/components/messages",
    forbidden: ["@/components/session", "@/components/settings"],
    allowList: [],
  },
  {
    name: "settings must not import messages",
    sourceDir: "src/components/settings",
    forbidden: ["@/components/messages"],
    allowList: [],
  },
  {
    name: "session must not import messages",
    sourceDir: "src/components/session",
    forbidden: ["@/components/messages"],
    allowList: [],
  },
  {
    name: "chat must only import messages through BlockRenderer",
    sourceDir: "src/components/chat",
    forbidden: ["@/components/messages"],
    allowList: [
      {
        file: "components/chat/BlockRenderer.tsx",
        importPrefix: "@/components/messages",
        reason: "BlockRenderer is the single designated entry point for message block renderers.",
      },
      {
        file: "components/chat/ConversationLane.tsx",
        importPrefix: "@/components/messages/ToolActivityGroup",
        reason:
          "ToolActivityGroup renders a grouped set of blocks (blocks[]). " +
          "BlockRenderer only accepts a single block. Architectural exception. " +
          "TODO: Consider lifting ToolActivityGroup into a chat-local wrapper or extending BlockRenderer.",
      },
    ],
  },
];

// ── Tests ────────────────────────────────────────────────────────────

for (const rule of RULES) {
  test(rule.name, () => {
    const dirPath = join(root, rule.sourceDir);
    const files = listTsFiles(dirPath);
    const violations = [];

    for (const file of files) {
      const relFile = relativeSrc(file);
      const content = readFileSync(file, "utf8");
      const imports = parseImports(content);

      for (const imp of imports) {
        if (!matchesAnyPrefix(imp.source, rule.forbidden)) continue;

        // Check allowList
        const allowed = rule.allowList.some(
          (a) => relFile === a.file && imp.source.startsWith(a.importPrefix)
        );
        if (allowed) continue;

        violations.push({
          file: relFile,
          import: imp.source,
          line: content.slice(0, content.indexOf(imp.raw)).split("\n").length,
        });
      }
    }

    if (violations.length > 0) {
      const summary = violations
        .map((v) => `  ${v.file}:${v.line}  imports  ${v.import}`)
        .join("\n");
      assert.fail(
        `${violations.length} boundary violation(s) found:\n${summary}`
      );
    }
  });
}

// ── Additional cross-boundary debt tracking (informational) ─────────

test("known cross-boundary imports are documented", () => {
  // These are documented architectural debts from the product layer map.
  // They do not fail the build but are tracked here so they do not silently multiply.
  const knownDebts = [
    {
      file: "src/components/chat/ConversationLane.tsx",
      import: "@/components/session/StartReadinessCard",
      reason:
        "StartReadinessCard lives in session/ for historical reasons but is used " +
        "by both chat/ConversationLane (empty state) and layout/EmptyWorkbench. " +
        "TODO: Move StartReadinessCard to a shared location (e.g. primitives/ or a new shared/ dir).",
    },
    {
      file: "src/components/layout/EmptyWorkbench.tsx",
      import: "@/components/session/StartReadinessCard",
      reason:
        "Same StartReadinessCard debt as above. layout/ should not import from session/.",
    },
    {
      file: "src/components/session/SessionView.tsx",
      import: "@/components/chat/ChatView",
      reason:
        "SessionView is the conversation shell that mounts ChatView + InputBar. " +
        "This is a structural coupling: SessionView (session/) orchestrates ChatView (chat/). " +
        "Acceptable because SessionView is the only session file that imports chat.",
    },
  ];

  for (const debt of knownDebts) {
    const filePath = join(root, debt.file);
    const content = readFileSync(filePath, "utf8");
    const found = content.includes(debt.import);
    assert.equal(
      found,
      true,
      `Known debt import not found: ${debt.file} should import ${debt.import}`
    );
  }
});
