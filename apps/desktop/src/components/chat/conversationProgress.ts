import type { BlockState } from "../../lib/protocol.ts";
import type { LiveProgressCandidate } from "./conversationTurnView.ts";

export const PROGRESS_LABEL_MINIMUM_MS = 600;

export interface StableProgressState {
  visible: LiveProgressCandidate | null;
  visibleSince: number;
  pending: LiveProgressCandidate | null;
  dueAt: number | null;
}

export function createStableProgressState(
  candidate: LiveProgressCandidate | null,
  now: number,
): StableProgressState {
  return { visible: candidate, visibleSince: now, pending: null, dueAt: null };
}

export function updateStableProgress(
  state: StableProgressState,
  candidate: LiveProgressCandidate | null,
  now: number,
  urgent = false,
): StableProgressState {
  if (!candidate) return createStableProgressState(null, now);
  if (!state.visible) return createStableProgressState(candidate, now);
  if (state.visible.id === candidate.id) return { ...state, pending: null, dueAt: null };

  const dueAt = state.visibleSince + PROGRESS_LABEL_MINIMUM_MS;
  if (urgent || now >= dueAt) return createStableProgressState(candidate, now);
  return { ...state, pending: candidate, dueAt };
}

export function flushStableProgress(state: StableProgressState, now: number): StableProgressState {
  if (!state.pending || state.dueAt === null || now < state.dueAt) return state;
  return createStableProgressState(state.pending, now);
}

export function deriveLiveProgressCandidate(blocks: BlockState[]): LiveProgressCandidate | null {
  const running = findLast(blocks, (block) => !block.isComplete && isProgressBlock(block));
  if (!running) return null;

  if (running.event_type === "thinking" || running.event_type === "pending") {
    return { id: "understanding", label: "正在理解任务" };
  }

  if (running.event_type === "text") {
    return { id: "answer:preparing", label: "正在整理回答" };
  }

  if (running.event_type === "tool_call") {
    const action = toolAction(running.metadata.tool_name);
    const name = safeInputBasename(running.metadata.tool_input);
    if (action && name) return { id: `${action.id}:${name}`, label: `${action.label} ${name}` };
    if (action) return { id: action.id, label: action.fallback };
  }

  if (running.event_type === "shell") {
    const verification = verificationAction(running.metadata.command);
    if (verification) return verification;
  }

  return { id: running.block_id || running.event_type, label: "正在执行操作" };
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

function verificationAction(value: unknown): LiveProgressCandidate | null {
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
    || block.event_type === "shell";
}

function findLast<T>(values: T[], predicate: (value: T) => boolean): T | null {
  for (let index = values.length - 1; index >= 0; index -= 1) {
    if (predicate(values[index])) return values[index];
  }
  return null;
}
