import type { SessionState } from "@/lib/protocol";
import {
  getModelLabel,
  getProviderLabel,
  getProviderModelLabel,
} from "@/lib/providers";

const UNTITLED_SESSION = "未命名对话";

export function getSessionTitle(session?: SessionState | null): string {
  if (!session) return "未选择对话";

  const firstUserMessage = session.blocks.find((block) => block.event_type === "user_message");
  const source = firstUserMessage?.content || session.agentType || UNTITLED_SESSION;
  const title = source
    .replace(/^\/\S+\s*/g, "")
    .replace(/@\S+/g, "")
    .replace(/\s+/g, " ")
    .trim();

  if (!title) return UNTITLED_SESSION;
  return truncateMiddle(title, 34);
}

export function getSessionStatus(session?: SessionState | null) {
  if (!session) return { label: "未开始", color: "#8C93A0" };
  if (session.streaming) return { label: "响应中", color: "#D4A853" };
  switch (session.status) {
    case "running":
      return { label: "运行中", color: "#4A9E6B" };
    case "error":
      return { label: "异常", color: "#D47777" };
    case "stopped":
      return { label: "已停止", color: "#8C93A0" };
    default:
      return { label: "未知", color: "#8C93A0" };
  }
}

export function getProjectDisplay(path?: string | null) {
  const normalized = normalizePath(path);
  if (!normalized) {
    return { name: "未选择项目", path: "选择一个具体项目目录后开始" };
  }

  return {
    name: normalized.split("/").filter(Boolean).pop() || normalized,
    path: normalized,
  };
}

export { getModelLabel, getProviderLabel, getProviderModelLabel };

function normalizePath(path?: string | null): string {
  const normalized = (path ?? "").trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  if (/^\/Users\/[^/]+$/.test(normalized) || /^\/home\/[^/]+$/.test(normalized)) return "";
  return normalized;
}

function truncateMiddle(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, maxLength - 1)}…`;
}
