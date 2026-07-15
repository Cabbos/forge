import { execFileSync as defaultExecFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { cwd } from "node:process";
import { fileURLToPath } from "node:url";

const HIGH_VALUE_AREAS = [
  {
    label: "Loop Runtime",
    paths: ["apps/desktop/src-tauri/src/loop_runtime/"],
  },
  {
    label: "Agent Runtime",
    paths: ["apps/desktop/src-tauri/src/agent/"],
  },
  {
    label: "Tooling & Permissions",
    paths: [
      "apps/desktop/src-tauri/src/harness/",
      "apps/desktop/src-tauri/src/executor/",
      "apps/desktop/src-tauri/src/gateway/",
      "apps/desktop/src-tauri/src/diagnostics/",
    ],
  },
  {
    label: "Desktop UI",
    paths: [
      "apps/desktop/src-tauri/src/protocol/",
      "apps/desktop/src/store/",
      "apps/desktop/src/components/",
    ],
  },
  {
    label: "Eval Runner",
    paths: ["apps/eval-runner/app/", "apps/eval-runner/eval_cases/"],
  },
  {
    label: "Acceptance",
    paths: ["scripts/acceptance.sh", "scripts/acceptance.test.mjs"],
  },
  {
    label: "Docs",
    paths: [
      "README.md",
      "apps/desktop/README.md",
      "CHANGELOG.md",
      "docs/superpowers/plans/",
      "docs/superpowers/specs/",
    ],
  },
];

const SYNC_PREFIX_PATTERN = /^(feat|fix|test|docs)(\(.+\))?:/;
const REFACTOR_PREFIX_PATTERN = /^refactor(\(.+\))?:/;
const DEPENDENCY_ONLY_PATTERN = /^chore\(deps\):/;
const LOCKFILE_NAMES = new Set(["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock"]);
const SKIP_PATH_PREFIXES = [
  ".forge/",
  "dist/",
  "target/",
  "artifacts/",
  "playwright-report/",
  "test-results/",
];
const DEFAULT_REPO_ROOT = fileURLToPath(new URL("..", import.meta.url)).replace(/\/+$/, "");
const DEFAULT_CONFIG_PATH = join(DEFAULT_REPO_ROOT, "docs/forge-sync/feishu-sync.config.json");
const DEFAULT_LOG_PATH = join(DEFAULT_REPO_ROOT, "docs/forge-sync/feishu-upgrade-log.md");

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function normalizePath(path) {
  return sanitizeRepoText(path).replace(/^\.?\//, "");
}

function isLockfile(path) {
  return LOCKFILE_NAMES.has(normalizePath(path));
}

function isSkipOnlyPath(path) {
  const normalized = normalizePath(path);
  return SKIP_PATH_PREFIXES.some((prefix) => normalized.startsWith(prefix)) || isLockfile(normalized);
}

function pathMatches(path, target) {
  const normalized = normalizePath(path);
  return target.endsWith("/") ? normalized.startsWith(target) : normalized === target;
}

function findAreas(files) {
  const areas = [];
  for (const area of HIGH_VALUE_AREAS) {
    if (files.some((file) => area.paths.some((path) => pathMatches(file, path)))) {
      areas.push(area.label);
    }
  }
  return areas;
}

export function sanitizeRepoText(text, repoRoot = cwd()) {
  let sanitized = String(text);
  const normalizedRoot = repoRoot.replace(/\/+$/, "");
  if (normalizedRoot) {
    sanitized = sanitized.replace(new RegExp(`${escapeRegExp(normalizedRoot)}/?`, "g"), "");
  }
  sanitized = sanitized.replace(/\/Users\/[^/\s]+\/project\/forge\/?/g, "");
  sanitized = sanitized.replace(/\/private\/var\/folders\/[^/\s]+\/[^/\s]+\/[^/\s]+\/forge\/?/g, "");
  sanitized = sanitized.replace(/file:\/\/[^\s]+\/forge\/?/g, "");
  return sanitized;
}

export function classifyCommit({ subject, files }) {
  const normalizedFiles = files.map(normalizePath);
  const areas = findAreas(normalizedFiles);
  const hasHighValuePath = areas.length > 0;
  const hasDocsOrAcceptance = areas.some((area) => area === "Docs" || area === "Acceptance");
  const hasSyncPrefix = SYNC_PREFIX_PATTERN.test(subject);
  const hasRuntimeRefactor = REFACTOR_PREFIX_PATTERN.test(subject) && hasHighValuePath;
  const dependencyOnly =
    DEPENDENCY_ONLY_PATTERN.test(subject) && normalizedFiles.every((file) => isLockfile(file));

  if (dependencyOnly) {
    return {
      action: "skip",
      reason: "dependency-only lockfile update",
      areas: [],
    };
  }

  if (normalizedFiles.length > 0 && normalizedFiles.every((file) => isSkipOnlyPath(file))) {
    return {
      action: "skip",
      reason: "skip-only generated or lockfile paths",
      areas: [],
    };
  }

  if ((hasSyncPrefix && hasHighValuePath) || hasRuntimeRefactor || hasDocsOrAcceptance) {
    return {
      action: "sync",
      reason: hasDocsOrAcceptance
        ? "docs or acceptance high-value paths"
        : "sync prefix with high-value paths",
      areas,
    };
  }

  return {
    action: "skip",
    reason: "no high-value sync signal",
    areas,
  };
}

export function generateUpgradeMarkdown({ sha, date, subject, files, classification }) {
  const shortSha = sanitizeRepoText(sha).slice(0, 7);
  const cleanSubject = sanitizeRepoText(subject);
  const cleanFiles = files.map(normalizePath);
  const areaList =
    classification.areas.length > 0
      ? classification.areas.map((area) => `- ${area}`).join("\n")
      : "- 未识别到明确产品域";
  const fileList =
    cleanFiles.length > 0
      ? cleanFiles.slice(0, 8).map((file) => `- ${file}`).join("\n")
      : "- 未识别到变更文件";
  const summary =
    classification.action === "sync"
      ? `本次 Forge 升级围绕「${cleanSubject}」展开，自动识别为需要同步的高价值变更。`
      : `本次 Forge 变更围绕「${cleanSubject}」展开，自动识别为暂不需要远程同步的变更。`;

  return [
    `## ${date} · ${shortSha} · ${cleanSubject}`,
    "",
    "### 升级摘要",
    summary,
    "",
    "### 影响范围",
    areaList,
    "",
    "### 关键改动",
    fileList,
    "",
    "### 验证证据",
    "- 自动同步脚本未推断出验证命令；请按需要补充。",
    "",
    "### 边界",
    "- 本条同步由本地 hook 生成，只记录高层升级，不包含完整 diff。",
    "",
  ].join("\n");
}

export function parseArgs(argv) {
  const parsed = {
    commit: "HEAD",
    dryRun: false,
    hook: "",
    setupFeishu: false,
    since: "",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--commit") {
      parsed.commit = argv[index + 1] ?? parsed.commit;
      index += 1;
    } else if (arg === "--since") {
      parsed.since = argv[index + 1] ?? "";
      index += 1;
    } else if (arg === "--hook") {
      parsed.hook = argv[index + 1] ?? "";
      index += 1;
    } else if (arg === "--dry-run") {
      parsed.dryRun = true;
    } else if (arg === "--setup-feishu") {
      parsed.setupFeishu = true;
    }
  }

  return parsed;
}

export function loadConfig(configPath = DEFAULT_CONFIG_PATH) {
  if (!existsSync(configPath)) {
    return {
      rootWikiUrl: "",
      upgradeLogUrl: "",
    };
  }

  const parsed = JSON.parse(readFileSync(configPath, "utf8"));
  return {
    rootWikiUrl: parsed.rootWikiUrl ?? "",
    upgradeLogUrl: parsed.upgradeLogUrl ?? "",
  };
}

export function appendLocalLog({ logPath = DEFAULT_LOG_PATH, markdown, status, reason = "", url = "" }) {
  mkdirSync(dirname(logPath), { recursive: true });
  const marker =
    status === "uploaded"
      ? `<!-- feishu-sync: uploaded url="${sanitizeRepoText(url)}" -->`
      : `<!-- feishu-sync: pending reason="${sanitizeRepoText(reason)}" -->`;
  const prefix = existsSync(logPath)
    ? ""
    : "# Forge Feishu Upgrade Log\n\nThis file is the local audit trail for valuable Forge upgrade summaries synced to Feishu.\n\n";

  appendFileSync(logPath, `${prefix}${marker}\n${sanitizeRepoText(markdown)}\n`);
}

export function extractJsonObject(output) {
  const text = String(output);

  for (let start = 0; start < text.length; start += 1) {
    if (text[start] !== "{") {
      continue;
    }

    let depth = 0;
    let inString = false;
    let escaped = false;

    for (let index = start; index < text.length; index += 1) {
      const char = text[index];
      if (inString) {
        if (escaped) {
          escaped = false;
        } else if (char === "\\") {
          escaped = true;
        } else if (char === "\"") {
          inString = false;
        }
        continue;
      }

      if (char === "\"") {
        inString = true;
      } else if (char === "{") {
        depth += 1;
      } else if (char === "}") {
        depth -= 1;
        if (depth === 0) {
          try {
            return JSON.parse(text.slice(start, index + 1));
          } catch {
            break;
          }
        }
      }
    }
  }

  return {};
}

function extractUrlFromText(text) {
  const match = String(text).match(/https:\/\/www\.feishu\.cn\/(?:wiki|docx)\/[A-Za-z0-9_-]+/);
  return match?.[0] ?? "";
}

function extractWikiToken(value) {
  const clean = String(value).trim();
  const match = clean.match(/\/wiki\/([^/?#]+)/);
  return match?.[1] ?? clean;
}

function writeConfig(configPath, config) {
  mkdirSync(dirname(configPath), { recursive: true });
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
}

export function setupFeishu({
  configPath = DEFAULT_CONFIG_PATH,
  rootWikiUrl,
  execFileSync = defaultExecFileSync,
  repoRoot = DEFAULT_REPO_ROOT,
} = {}) {
  const existing = loadConfig(configPath);
  const rootUrl = rootWikiUrl || existing.rootWikiUrl;
  if (existing.upgradeLogUrl) {
    return {
      ok: true,
      upgradeLogUrl: existing.upgradeLogUrl,
    };
  }
  if (!rootUrl) {
    return {
      ok: false,
      reason: "missing_root_wiki_url",
    };
  }

  const output = execFileSync(
    "lark-cli",
    [
      "docs",
      "+create",
      "--wiki-node",
      extractWikiToken(rootUrl),
      "--title",
      "Forge 升级同步",
      "--markdown",
      "# Forge 升级同步\n\n这里自动汇总本地 Hook 识别出的高价值 Forge 升级记录。\n",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );
  const parsed = extractJsonObject(output);
  const upgradeLogUrl =
    parsed.url ?? parsed.wikiUrl ?? parsed.wiki_url ?? parsed.documentUrl ?? extractUrlFromText(output);
  if (!upgradeLogUrl) {
    return {
      ok: false,
      reason: "missing_created_doc_url",
    };
  }

  writeConfig(configPath, {
    rootWikiUrl: rootUrl,
    upgradeLogUrl,
  });

  return {
    ok: true,
    upgradeLogUrl,
  };
}

export function syncToFeishu({
  config,
  markdown,
  execFileSync = defaultExecFileSync,
  repoRoot = DEFAULT_REPO_ROOT,
} = {}) {
  if (!config?.upgradeLogUrl) {
    return {
      ok: false,
      reason: "missing_upgrade_log_url",
    };
  }

  try {
    execFileSync(
      "lark-cli",
      [
        "docs",
        "+update",
        "--doc",
        config.upgradeLogUrl,
        "--mode",
        "append",
        "--markdown",
        sanitizeRepoText(markdown),
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
      },
    );
    return {
      ok: true,
      url: config.upgradeLogUrl,
    };
  } catch (error) {
    return {
      ok: false,
      reason: "upload_failed",
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export function readCommitMetadata({
  commit = "HEAD",
  since = "",
  execFileSync = defaultExecFileSync,
  repoRoot = DEFAULT_REPO_ROOT,
} = {}) {
  const metadata = execFileSync(
    "git",
    ["show", "-s", "--format=%H%n%s%n%cd", "--date=short", commit],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  )
    .trimEnd()
    .split("\n");
  const [sha = "", subject = "", date = ""] = metadata;
  const fileArgs = since
    ? ["diff", "--name-only", since, commit]
    : ["diff-tree", "--no-commit-id", "--name-only", "-r", commit];
  const files = execFileSync("git", fileArgs, {
    cwd: repoRoot,
    encoding: "utf8",
  })
    .split("\n")
    .map((file) => file.trim())
    .filter(Boolean);

  return {
    date,
    files,
    sha,
    subject,
  };
}

export async function runCli(argv = process.argv.slice(2), deps = {}) {
  const args = parseArgs(argv);
  const repoRoot = deps.repoRoot ?? DEFAULT_REPO_ROOT;
  const configPath = deps.configPath ?? DEFAULT_CONFIG_PATH;
  const logPath = deps.logPath ?? DEFAULT_LOG_PATH;
  const execFileSync = deps.execFileSync ?? defaultExecFileSync;
  const stdout = deps.stdout ?? process.stdout;
  const stderr = deps.stderr ?? process.stderr;

  if (process.env.FORGE_FEISHU_SYNC_SKIP === "1") {
    return {
      ok: true,
      skipped: true,
      reason: "FORGE_FEISHU_SYNC_SKIP",
    };
  }

  if (args.setupFeishu) {
    const result = setupFeishu({
      configPath,
      execFileSync,
      repoRoot,
    });
    if (result.ok) {
      stdout.write(`[forge-feishu] setup complete: ${result.upgradeLogUrl}\n`);
    } else {
      stderr.write(`[forge-feishu] setup failed: ${result.reason}\n`);
    }
    return result;
  }

  const metadata = readCommitMetadata({
    commit: args.commit,
    since: args.since,
    execFileSync,
    repoRoot,
  });
  const classification = classifyCommit({
    subject: metadata.subject,
    files: metadata.files,
  });
  const markdown = generateUpgradeMarkdown({
    ...metadata,
    classification,
  });

  if (args.dryRun) {
    stdout.write(
      JSON.stringify(
        {
          action: classification.action,
          areas: classification.areas,
          reason: classification.reason,
        },
        null,
        2,
      ),
    );
    stdout.write("\n\n");
    stdout.write(markdown);
    return {
      ok: true,
      dryRun: true,
      classification,
      markdown,
    };
  }

  if (classification.action === "skip") {
    if (!args.hook) {
      stdout.write(`[forge-feishu] skipped: ${classification.reason}\n`);
    }
    return {
      ok: true,
      skipped: true,
      classification,
    };
  }

  const config = loadConfig(configPath);
  const upload = syncToFeishu({
    config,
    execFileSync,
    markdown,
    repoRoot,
  });
  appendLocalLog({
    logPath,
    markdown,
    status: upload.ok ? "uploaded" : "pending",
    reason: upload.reason,
    url: upload.url,
  });

  if (!args.hook && !upload.ok) {
    stderr.write(`[forge-feishu] Feishu upload failed: ${upload.reason}; wrote local pending log.\n`);
  }

  return {
    ok: args.hook ? true : upload.ok,
    pending: !upload.ok,
    uploaded: upload.ok,
    classification,
    markdown,
  };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  runCli().then((result) => {
    if (!result.ok) {
      process.exitCode = 1;
    }
  });
}
