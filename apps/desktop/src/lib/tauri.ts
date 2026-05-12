import { invoke } from "@tauri-apps/api/core";

const WORKING_DIR_KEY = "tui-to-gui-working-dir";
const FILE_OPEN_TEMPLATE_KEY = "tui-to-gui-file-open-template";
const DEFAULT_FILE_OPEN_TEMPLATE = "vscode://file/{path}{lineSuffix}";

export interface SessionCreated { session_id: string }

export interface SessionInfo {
  id: string; provider: string; model: string;
  status: string; created_at: string;
}

export interface FilePreviewLine {
  number: number;
  content: string;
  is_target: boolean;
}

export interface FilePreview {
  path: string;
  display_path: string;
  requested_line: number | null;
  start_line: number;
  total_lines: number;
  lines: FilePreviewLine[];
}

export async function createSession(workingDir: string, apiKey: string, model: string): Promise<SessionCreated> {
  rememberWorkingDir(workingDir);
  return invoke<SessionCreated>("create_session", {
    workingDir, apiKey: apiKey || "", model,
  });
}

export async function sendInput(sessionId: string, text: string): Promise<void> {
  return invoke("send_input", { sessionId, text });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

export async function getDefaultWorkingDir(): Promise<string> {
  return invoke("get_default_working_dir");
}

export async function confirmResponse(blockId: string, approved: boolean): Promise<void> {
  return invoke("confirm_response", { blockId, approved });
}

export interface KeyStatus { provider: string; set: boolean; preview: string }

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  return invoke("get_api_key_status");
}

export async function setApiKey(provider: string, key: string): Promise<void> {
  return invoke("set_api_key", { provider, key });
}

// Plugin exports (kept for existing UI components)
export interface PluginEntry {
  id: string; name: string; description: string;
  plugin_type: string; agent: string; category: string;
  status: unknown; config_schema?: unknown; current_config?: unknown;
  homepage?: string; author?: string;
}
export async function listPlugins(agent: string): Promise<PluginEntry[]> { return invoke("list_plugins", { agent }); }
export async function discoverPlugins(agent: string): Promise<PluginEntry[]> { return invoke("discover_plugins", { agent }); }
export async function installPlugin(pluginId: string, agent: string, config?: unknown): Promise<void> { return invoke("install_plugin", { pluginId, agent, config: config ?? null }); }
export async function uninstallPlugin(pluginId: string, agent: string): Promise<void> { return invoke("uninstall_plugin", { pluginId, agent }); }
export async function togglePlugin(pluginId: string, agent: string, enabled: boolean): Promise<void> { return invoke("toggle_plugin", { pluginId, agent, enabled }); }

// Capability IPC
export interface CapabilityInfo {
  id: string;
  name: string;
  description: string;
  kind: string;
  source: string;
  version: string;
  enabled: boolean;
}

export async function listCapabilities(): Promise<CapabilityInfo[]> {
  return invoke("list_capabilities");
}

export async function toggleCapability(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_capability", { capabilityId: id, enabled });
}

/** Search workspace files for @ autocomplete */
export async function searchWorkspaceFiles(query: string): Promise<string[]> {
  return invoke("search_workspace_files", { query });
}

/** Install a skill from GitHub (owner/repo) */
export async function installSkill(repo: string): Promise<CapabilityInfo> {
  return invoke("install_skill", { repo });
}

/** Open a file in the system editor at an optional line number */
export async function openFile(path: string, line?: number, sessionId?: string): Promise<void> {
  if (!hasTauriRuntime() && openFileViaUrlScheme(path, line)) return;

  try {
    return await invoke("open_file", { path, line: line ?? null, sessionId: sessionId ?? null });
  } catch (error) {
    if (openFileViaUrlScheme(path, line)) return;
    throw error;
  }
}

/** Read a small, beginner-friendly preview around a file reference */
export async function previewFile(path: string, line?: number, sessionId?: string): Promise<FilePreview> {
  return invoke("preview_file", { path, line: line ?? null, context: 40, sessionId: sessionId ?? null });
}

function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

export function rememberWorkingDir(workingDir: string) {
  if (typeof window === "undefined" || !workingDir.trim()) return;
  window.localStorage.setItem(WORKING_DIR_KEY, workingDir.trim());
}

export function getRememberedWorkingDir(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(WORKING_DIR_KEY);
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
  ].reduce((url, [token, value]) => url.split(token).join(value), template);
}

function resolveFallbackPath(path: string): string | null {
  const trimmed = path.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("/")) return trimmed;

  const workingDir = window.localStorage.getItem(WORKING_DIR_KEY);
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
