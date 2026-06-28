import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(SCRIPT_DIR, "..");

const BLOCKED_PREFIXES = [
  "test-results/",
  "playwright-report/",
  "artifacts/",
  "dist/",
  "src-tauri/target/",
  ".forge/",
  ".ai-studio/",
  ".agents/",
  ".claude/",
  ".playwright-cli/",
  ".superpowers/",
  ".gitnexus/",
];

const BLOCKED_FILES = new Set([".mcp.json", "skills-lock.json"]);

const FRONTEND_ROOTS = ["src/", "e2e/", "scripts/"];
const FRONTEND_FILES = new Set([
  "components.json",
  "package.json",
  "package-lock.json",
  "postcss.config.js",
  "tailwind.config.js",
  "tsconfig.json",
  "vite.config.ts",
]);
const FRONTEND_EXTENSIONS = [
  ".css",
  ".cjs",
  ".html",
  ".js",
  ".jsx",
  ".json",
  ".mjs",
  ".ts",
  ".tsx",
];

const RUST_FILES = new Set(["src-tauri/Cargo.lock", "src-tauri/Cargo.toml"]);
const PROTOCOL_FILES = new Set([
  "src-tauri/src/protocol/events.rs",
  "src/lib/protocol.ts",
  "src/store/event-dispatch.ts",
  "src/store/blocks.ts",
  "scripts/check-protocol-sync.mjs",
]);
const DESKTOP_APP_PREFIX = "apps/desktop/";

const CHECKS = {
  conversationStyle: {
    command: "npm",
    args: ["run", "check:conversation-style"],
  },
  frontendArchitecture: {
    command: "npm",
    args: ["run", "check:frontend-architecture"],
  },
  typescript: {
    command: "npx",
    args: ["tsc", "--noEmit"],
  },
  protocolSync: {
    command: "npm",
    args: ["run", "check:protocol"],
  },
  rustfmt: {
    command: "cargo",
    args: ["fmt", "--manifest-path", "src-tauri/Cargo.toml", "--check"],
  },
  clippy: {
    command: "cargo",
    args: [
      "clippy",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--all-targets",
      "--",
      "-D",
      "warnings",
    ],
  },
};

export function describeCommand(command) {
  return [command.command, ...command.args].join(" ");
}

export function buildPreCommitPlan(stagedFiles) {
  const normalizedFiles = stagedFiles.map(normalizePath).filter(Boolean);
  const blockedFiles = normalizedFiles.filter(isBlockedArtifact);
  const touchesFrontend = normalizedFiles.some(isFrontendFile);
  const touchesRust = normalizedFiles.some(isRustFile);
  const touchesProtocol = normalizedFiles.some(isProtocolFile);
  const commands = [];

  if (touchesFrontend) {
    commands.push(CHECKS.conversationStyle, CHECKS.frontendArchitecture, CHECKS.typescript);
  }

  if (touchesProtocol) {
    commands.push(CHECKS.protocolSync);
  }

  if (touchesRust) {
    commands.push(CHECKS.rustfmt, CHECKS.clippy);
  }

  return { blockedFiles, commands };
}

function normalizePath(filePath) {
  const normalized = filePath.replaceAll("\\", "/").replace(/^\.\//, "");
  return normalized.startsWith(DESKTOP_APP_PREFIX)
    ? normalized.slice(DESKTOP_APP_PREFIX.length)
    : normalized;
}

function isProtocolFile(filePath) {
  return (
    PROTOCOL_FILES.has(filePath) || filePath.startsWith("src-tauri/src/protocol/")
  );
}

function isBlockedArtifact(filePath) {
  return (
    BLOCKED_FILES.has(filePath) ||
    BLOCKED_PREFIXES.some((prefix) => filePath.startsWith(prefix))
  );
}

function isFrontendFile(filePath) {
  return (
    FRONTEND_FILES.has(filePath) ||
    (FRONTEND_ROOTS.some((root) => filePath.startsWith(root)) &&
      FRONTEND_EXTENSIONS.some((extension) => filePath.endsWith(extension)))
  );
}

function isRustFile(filePath) {
  return (
    RUST_FILES.has(filePath) ||
    (filePath.startsWith("src-tauri/") && filePath.endsWith(".rs"))
  );
}

function readStagedFiles() {
  const output = execFileSync(
    "git",
    ["diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"],
    { encoding: "utf8" },
  );

  return output.split("\0").filter(Boolean);
}

function runCommand(command) {
  execFileSync(command.command, command.args, { stdio: "inherit", cwd: PROJECT_ROOT });
}

function runCli() {
  const stagedFiles = readStagedFiles();
  const plan = buildPreCommitPlan(stagedFiles);

  if (plan.blockedFiles.length > 0) {
    console.error("[pre-commit] Refusing to commit local/generated files:");
    for (const filePath of plan.blockedFiles) {
      console.error(`  - ${filePath}`);
    }
    console.error("[pre-commit] Unstage these files, then commit again.");
    return 1;
  }

  if (plan.commands.length === 0) {
    console.log("[pre-commit] No staged Forge code paths need checks.");
    return 0;
  }

  for (const command of plan.commands) {
    console.log(`[pre-commit] ${describeCommand(command)}`);
    runCommand(command);
  }

  console.log("[pre-commit] Checks passed.");
  return 0;
}

const invokedPath = process.argv[1] ? pathToFileURL(process.argv[1]).href : "";

if (import.meta.url === invokedPath) {
  process.exitCode = runCli();
}
