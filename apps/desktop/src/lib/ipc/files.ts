import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { FilePreview } from "./types";
import { getRememberedWorkingDir } from "./app";

const FILE_OPEN_TEMPLATE_KEY = "forge-file-open-template";
const DEFAULT_FILE_OPEN_TEMPLATE = "vscode://file/{path}{lineSuffix}";

export async function openFile(
  path: string,
  line?: number,
  sessionId?: string,
  workingDir?: string | null,
): Promise<void> {
  if (!hasTauriRuntime() && openFileViaUrlScheme(path, line)) return;

  try {
    return await invoke("open_file", {
      path,
      line: line ?? null,
      sessionId: sessionId ?? null,
      workingDir: workingDir ?? null,
    });
  } catch (error) {
    if (openFileViaUrlScheme(path, line)) return;
    throw error;
  }
}

export async function previewFile(
  path: string,
  line?: number,
  sessionId?: string,
  workingDir?: string | null,
): Promise<FilePreview> {
  return invoke("preview_file", {
    path,
    line: line ?? null,
    context: 40,
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

function openFileViaUrlScheme(path: string, line?: number): boolean {
  if (typeof window === "undefined") return false;

  const absolutePath = resolveFallbackPath(path);
  if (!absolutePath) return false;

  const template = getFileOpenTemplate();
  if (!template) return false;

  window.location.href = formatFileOpenUrl(template, absolutePath, line);
  return true;
}

function getFileOpenTemplate(): string | null {
  const envTemplate = import.meta.env.VITE_OPEN_FILE_URL_TEMPLATE as string | undefined;
  const template =
    window.localStorage.getItem(FILE_OPEN_TEMPLATE_KEY) ||
    envTemplate ||
    DEFAULT_FILE_OPEN_TEMPLATE;
  const normalized = template.trim();

  if (!normalized || ["none", "off", "disabled"].includes(normalized.toLowerCase())) {
    return null;
  }

  return normalized;
}

function formatFileOpenUrl(template: string, path: string, line?: number): string {
  const normalizedPath = path.replace(/\\/g, "/");
  const lineValue = line ? String(line) : "";
  const lineSuffix = line ? `:${line}` : "";

  return [
    ["{path}", encodeURI(normalizedPath)],
    ["{rawPath}", normalizedPath],
    ["{pathEncoded}", encodeURIComponent(normalizedPath)],
    ["{line}", lineValue],
    ["{lineSuffix}", lineSuffix],
  ].reduce((url, [token, value]) => url.split(token as string).join(value as string), template);
}

function resolveFallbackPath(path: string): string | null {
  const trimmed = path.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("/")) return trimmed;

  const workingDir = getRememberedWorkingDir();
  if (!workingDir) return null;

  if (trimmed.startsWith("@/")) {
    return joinPath(workingDir, "src", trimmed.slice(2));
  }

  return joinPath(workingDir, trimmed);
}

function joinPath(...parts: string[]): string {
  return parts
    .map((part, index) => {
      if (index === 0) return part.replace(/\/+$/, "");
      return part.replace(/^\/+|\/+$/g, "");
    })
    .filter(Boolean)
    .join("/");
}
