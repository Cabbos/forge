import type { BlockState } from "../../lib/protocol.ts";

export const PROGRESS_INITIAL_DELAY_MS = 240;
export const PROGRESS_LABEL_MINIMUM_MS = 600;

export type LiveProgressStage =
  | "analyzing"
  | "discovering"
  | "modifying"
  | "verifying"
  | "answering"
  | "waiting";

export interface LiveProgressCandidate {
  id: LiveProgressStage;
  label: string;
  motion: "live" | "paused";
  urgent?: boolean;
}

const LIVE_PROGRESS_CANDIDATES = {
  analyzing: { id: "analyzing", label: "正在分析", motion: "live" },
  discovering: { id: "discovering", label: "正在查找相关内容", motion: "live" },
  modifying: { id: "modifying", label: "正在进行修改", motion: "live" },
  verifying: { id: "verifying", label: "正在验证结果", motion: "live" },
  answering: { id: "answering", label: "正在生成答复", motion: "live" },
  waiting: { id: "waiting", label: "等待你的确认", motion: "paused", urgent: true },
} satisfies Record<LiveProgressStage, LiveProgressCandidate>;

export interface StableProgressState {
  visible: LiveProgressCandidate | null;
  visibleSince: number;
  pending: LiveProgressCandidate | null;
  dueAt: number | null;
  hasPresented: boolean;
}

export function createStableProgressState(
  candidate: LiveProgressCandidate | null,
  now: number,
): StableProgressState {
  if (!candidate || candidate.id === "answering") return emptyProgressState(now);
  if (candidate.urgent === true) return presentedProgressState(candidate, now);
  return {
    visible: null,
    visibleSince: now,
    pending: candidate,
    dueAt: now + PROGRESS_INITIAL_DELAY_MS,
    hasPresented: false,
  };
}

export function updateStableProgress(
  state: StableProgressState,
  candidate: LiveProgressCandidate | null,
  now: number,
  urgent = false,
): StableProgressState {
  if (!candidate) {
    if (state.hasPresented && state.visible) {
      return { ...state, pending: null, dueAt: null };
    }
    return emptyProgressState(now);
  }
  if (state.hasPresented && state.visible?.id === candidate.id) {
    return { ...state, visible: candidate, pending: null, dueAt: null };
  }
  if (urgent || candidate.urgent === true) return presentedProgressState(candidate, now);
  if (state.visible?.id === "waiting") return presentedProgressState(candidate, now);

  if (!state.hasPresented) {
    if (candidate.id === "answering") return emptyProgressState(now);
    return {
      visible: null,
      visibleSince: state.visibleSince,
      pending: candidate,
      dueAt: state.dueAt ?? now + PROGRESS_INITIAL_DELAY_MS,
      hasPresented: false,
    };
  }

  if (!state.visible) return createStableProgressState(candidate, now);

  const dueAt = state.visibleSince + PROGRESS_LABEL_MINIMUM_MS;
  if (now >= dueAt) return presentedProgressState(candidate, now);
  return { ...state, pending: candidate, dueAt };
}

export function flushStableProgress(state: StableProgressState, now: number): StableProgressState {
  if (!state.pending || state.dueAt === null || now < state.dueAt) return state;
  return presentedProgressState(state.pending, now);
}

export function analyzingProgressCandidate(): LiveProgressCandidate {
  return LIVE_PROGRESS_CANDIDATES.analyzing;
}

export function answeringProgressCandidate(): LiveProgressCandidate {
  return LIVE_PROGRESS_CANDIDATES.answering;
}

export function waitingProgressCandidate(): LiveProgressCandidate {
  return LIVE_PROGRESS_CANDIDATES.waiting;
}

export function progressCandidateForBlock(block: BlockState): LiveProgressCandidate {
  if (block.event_type === "confirm_ask" && isUnresolvedConfirmation(block)) {
    return waitingProgressCandidate();
  }

  if (block.event_type === "text") return answeringProgressCandidate();

  if (block.event_type === "tool_call") {
    const toolName = block.metadata.tool_name;
    if (typeof toolName === "string" && DISCOVERY_TOOL_NAMES.has(toolName)) {
      return LIVE_PROGRESS_CANDIDATES.discovering;
    }
    if (typeof toolName === "string" && MODIFICATION_TOOL_NAMES.has(toolName)) {
      return LIVE_PROGRESS_CANDIDATES.modifying;
    }
    return analyzingProgressCandidate();
  }

  if (block.event_type === "diff_view") return LIVE_PROGRESS_CANDIDATES.modifying;

  if (block.event_type === "shell" && isSafeVerificationCommand(block.metadata.command)) {
    return LIVE_PROGRESS_CANDIDATES.verifying;
  }

  return analyzingProgressCandidate();
}

export function deriveLiveProgressCandidate(blocks: BlockState[]): LiveProgressCandidate | null {
  const running = findLast(
    blocks,
    (block) => !block.isComplete && isProgressBlock(block) && !isResolvedConfirmation(block),
  );
  if (!running) return null;
  return progressCandidateForBlock(running);
}

export function deriveCompletedProcessLabel(block: BlockState) {
  if (block.event_type === "tool_call" || block.event_type === "tool_call_result") {
    const action = toolAction(block.metadata.tool_name);
    const name = safeInputBasename(block.metadata.tool_input);
    if (action?.id === "read") return name ? `已查看 ${name}` : "已查看相关内容";
    if (action?.id === "search") return name ? `已查找 ${name}` : "已查找相关内容";
    if (action?.id === "edit") return name ? `已调整 ${name}` : "已调整相关内容";
    return "已执行操作";
  }

  if (block.event_type === "shell") {
    const verification = verificationAction(block.metadata.command);
    if (verification?.id === "verify:build") return "已验证构建";
    if (verification?.id === "verify:test") return "已运行测试";
    if (verification?.id === "verify:type") return "已检查类型";
    if (verification?.id === "verify:lint") return "已检查代码";
    if (verification) return "已验证结果";
    return "已执行命令";
  }

  if (block.event_type === "diff_view") {
    const name = safeObjectName(block.metadata.file_path);
    return name ? `已更新 ${name}` : "已更新文件";
  }

  return "已执行操作";
}

function safeInputBasename(value: unknown) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const input = value as Record<string, unknown>;
  const path = typeof input.path === "string" ? input.path : input.file_path;
  if (typeof path !== "string") return null;
  return safeObjectName(path);
}

function safeObjectName(value: unknown) {
  if (typeof value !== "string") return null;
  const normalized = value.trim().split(/[?#]/, 1)[0].replace(/\\/g, "/").replace(/\/+$/, "");
  const name = normalized.split("/").pop()?.trim() ?? "";
  if (!name || isSensitiveName(name)) return null;
  return truncateObjectName(name);
}

function toolAction(value: unknown) {
  if (typeof value !== "string") return null;
  if (["read_file", "read"].includes(value)) {
    return { id: "read", label: "正在查看", fallback: "正在查看相关内容" };
  }
  if (["search_content", "grep", "search_files", "glob"].includes(value)) {
    return { id: "search", label: "正在查找", fallback: "正在查找相关内容" };
  }
  if (["write_file", "write", "edit"].includes(value)) {
    return { id: "edit", label: "正在调整", fallback: "正在调整相关内容" };
  }
  return null;
}

function isSensitiveName(name: string) {
  return /^(?:\.env(?:\..*)?|\.npmrc|\.pypirc|id_[rd]sa(?:\.pub)?)$/i.test(name)
    || /(?:^|[._-])(?:secret|token|password|passwd|credential|private[-_]?key|api[-_]?key)(?:[._-]|$)/i.test(name);
}

function truncateObjectName(name: string, maxLength = 36) {
  if (name.length <= maxLength) return name;
  const extensionIndex = name.lastIndexOf(".");
  const extension = extensionIndex > 0 ? name.slice(extensionIndex) : "";
  if (extension.length > 0 && extension.length <= 10) {
    return `${name.slice(0, maxLength - extension.length - 1)}…${extension}`;
  }
  return `${name.slice(0, maxLength - 1)}…`;
}

function verificationAction(value: unknown): { id: string; label: string } | null {
  if (typeof value !== "string") return null;
  if (/\b(?:typecheck|tsc)\b/i.test(value)) return { id: "verify:type", label: "正在检查类型" };
  if (/\b(?:test|vitest|playwright)\b/i.test(value)) return { id: "verify:test", label: "正在运行测试" };
  if (/\bbuild\b/i.test(value)) return { id: "verify:build", label: "正在验证构建" };
  if (/\blint\b/i.test(value)) return { id: "verify:lint", label: "正在检查代码" };
  if (/\bcheck\b/i.test(value)) return { id: "verify:check", label: "正在验证结果" };
  return null;
}

function isProgressBlock(block: BlockState) {
  return block.event_type === "thinking"
    || block.event_type === "pending"
    || block.event_type === "text"
    || block.event_type === "tool_call"
    || block.event_type === "shell"
    || block.event_type === "diff_view"
    || block.event_type === "confirm_ask";
}

function isResolvedConfirmation(block: BlockState) {
  return block.event_type === "confirm_ask" && !isUnresolvedConfirmation(block);
}

function isUnresolvedConfirmation(block: BlockState) {
  return block.metadata.confirmed !== true && block.metadata.confirm_interrupted !== true;
}

function isSafeVerificationCommand(value: unknown) {
  if (typeof value !== "string") return false;
  const command = value.trim();
  if (!command || hasShellControlOperator(command)) return false;

  const [program, ...args] = command.split(/\s+/);
  if (program === "cargo") return args[0] === "check" || args[0] === "test";

  if (program === "npm") {
    return args[0] === "test" || (args[0] === "run" && isVerificationScript(args[1]));
  }

  if (program === "pnpm") {
    if (args[0] === "exec") return isDirectVerificationRunner(args.slice(1));
    return args[0] === "test" || (args[0] === "run" && isVerificationScript(args[1]));
  }

  if (program === "yarn") {
    if (args[0] === "run") return isVerificationScript(args[1]);
    return isVerificationScript(args[0]);
  }

  if (program === "bun") {
    return args[0] === "test" || (args[0] === "run" && isVerificationScript(args[1]));
  }

  if (program === "npx") return isDirectVerificationRunner(args);
  return isDirectVerificationRunner([program, ...args]);
}

function isVerificationScript(value: string | undefined) {
  const base = value?.split(":", 1)[0];
  return base === "build"
    || base === "test"
    || base === "check"
    || base === "lint"
    || base === "typecheck";
}

function isDirectVerificationRunner(parts: string[]) {
  const [runner, subcommand] = parts;
  if (["tsc", "vue-tsc", "svelte-check", "vitest", "jest", "eslint"].includes(runner)) {
    return true;
  }
  if (runner === "vite") return subcommand === "build";
  if (runner === "playwright") return subcommand === "test";
  return false;
}

function hasShellControlOperator(command: string) {
  let quote: "'" | '"' | null = null;
  let escaped = false;

  for (let index = 0; index < command.length; index += 1) {
    const character = command[index];

    if (quote === "'") {
      if (character === "'") quote = null;
      continue;
    }

    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") {
      escaped = true;
      continue;
    }

    if (character === '"') {
      quote = quote === '"' ? null : '"';
      continue;
    }
    if (character === "'" && quote === null) {
      quote = "'";
      continue;
    }

    const commandSubstitution = character === "$" && command[index + 1] === "(";
    if ((quote === null || quote === '"') && (character === "`" || commandSubstitution)) {
      return true;
    }
    if (
      quote === null
      && (character === "&"
        || character === ";"
        || character === "|"
        || character === ">"
        || character === "<"
        || character === "\n"
        || character === "\r")
    ) {
      return true;
    }
  }

  return quote !== null || escaped;
}

function emptyProgressState(now: number): StableProgressState {
  return {
    visible: null,
    visibleSince: now,
    pending: null,
    dueAt: null,
    hasPresented: false,
  };
}

function presentedProgressState(
  candidate: LiveProgressCandidate,
  now: number,
): StableProgressState {
  return {
    visible: candidate,
    visibleSince: now,
    pending: null,
    dueAt: null,
    hasPresented: true,
  };
}

const DISCOVERY_TOOL_NAMES = new Set([
  "read_file",
  "read",
  "search_content",
  "grep",
  "search_files",
  "glob",
]);

const MODIFICATION_TOOL_NAMES = new Set([
  "write_file",
  "write_to_file",
  "write",
  "edit_file",
  "edit",
  "apply_patch",
  "create_file",
  "delete_file",
  "move_file",
]);

function findLast<T>(values: T[], predicate: (value: T) => boolean): T | null {
  for (let index = values.length - 1; index >= 0; index -= 1) {
    if (predicate(values[index])) return values[index];
  }
  return null;
}
