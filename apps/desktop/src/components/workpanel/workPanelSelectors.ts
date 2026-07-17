import type { WorkPanelLauncherAction, WorkPanelTab } from "./workPanelTypes.ts";

export interface WorkPanelLauncherActionDefinition {
  id: WorkPanelLauncherAction;
  label: string;
  shortcut?: string;
}

export const WORK_PANEL_LAUNCHER_ACTIONS: readonly WorkPanelLauncherActionDefinition[] = [
  { id: "review", label: "审阅", shortcut: "⌃⇧G" },
  { id: "terminal", label: "终端" },
  { id: "preview", label: "预览", shortcut: "⌘T" },
  { id: "files", label: "文件", shortcut: "⌘P" },
  { id: "subtasks", label: "子任务", shortcut: "⌥⌘S" },
];

export function createReviewTab(taskId: string): WorkPanelTab {
  return {
    kind: "review",
    id: `review:${taskId}`,
    label: "审阅 · 当前改动",
    taskId,
  };
}

export function createTerminalTab(taskId: string): WorkPanelTab {
  return {
    kind: "terminal",
    id: `terminal:${taskId}`,
    label: "终端",
    taskId,
  };
}

export function createFileTab(path: string): WorkPanelTab {
  const cleanPath = path.trim();
  return {
    kind: "file",
    id: `file:${cleanPath}`,
    label: basename(cleanPath),
    path: cleanPath,
  };
}

export function createPreviewFileTab(path: string): WorkPanelTab {
  const cleanPath = path.trim();
  return {
    kind: "preview",
    id: `preview-file:${cleanPath}`,
    label: basename(cleanPath),
    target: { type: "file", path: cleanPath },
  };
}

export function createPreviewUrlTab(
  value: string,
): Extract<WorkPanelTab, { kind: "preview" }> | null {
  const url = normalizePreviewUrl(value);
  if (!url) return null;
  return {
    kind: "preview",
    id: `preview:${url}`,
    label: previewUrlLabel(url),
    target: { type: "url", url },
  };
}

export function createSubtaskTab(taskId: string, subtaskId: string, label: string): WorkPanelTab {
  return {
    kind: "subtask",
    id: `subtask:${subtaskId}`,
    label: label.trim() || "子任务",
    taskId,
    subtaskId,
  };
}

export function isAllowedPreviewUrl(value: string): boolean {
  return normalizePreviewUrl(value) !== null;
}

export function normalizePreviewUrl(value: string): string | null {
  try {
    const url = new URL(value.trim());
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    if (url.username || url.password) return null;
    const hostname = url.hostname.toLowerCase();
    const loopback = hostname === "localhost"
      || hostname.endsWith(".localhost")
      || hostname === "127.0.0.1"
      || hostname === "[::1]";
    if (!loopback) return null;
    if (url.pathname === "/" && !url.search && !url.hash) {
      return url.origin;
    }
    return url.toString();
  } catch {
    return null;
  }
}

function previewUrlLabel(value: string): string {
  const url = new URL(value);
  return url.port ? `${url.hostname}:${url.port}` : url.hostname;
}

function basename(path: string): string {
  const cleanPath = path.replace(/\/+$/, "");
  return cleanPath.split("/").pop() || cleanPath || "文件";
}
