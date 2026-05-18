import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ForgeWikiPage,
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  MemoryPatch,
  MemoryScope,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  WikiMemory,
  WorkflowOverrideAction,
  WorkflowState,
} from "./protocol";

const WORKING_DIR_KEY = "forge-working-dir";
const FILE_OPEN_TEMPLATE_KEY = "forge-file-open-template";
const DEFAULT_FILE_OPEN_TEMPLATE = "vscode://file/{path}{lineSuffix}";

export interface SessionCreated {
  session_id: string;
  provider?: string;
  model?: string;
  missing_api_key?: boolean;
}

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

export interface ProjectRuntimeStatus {
  working_dir: string;
  has_package_json: boolean;
  package_manager: string;
  dev_script: string | null;
  command: string | null;
  port: number;
  url: string;
  running: boolean;
  managed: boolean;
  pid: number | null;
  can_start: boolean;
  can_stop: boolean;
  can_open: boolean;
  message: string;
  logs: string[];
}

export interface ProjectCheckpoint {
  id: string;
  created_at: number;
  head: string;
  status: string;
}

export interface ProjectCheckpointStatus {
  working_dir: string;
  is_git_repo: boolean;
  dirty: boolean;
  last_checkpoint: ProjectCheckpoint | null;
  message: string;
}

export interface McpContextResource {
  server_id: string;
  uri: string;
  name: string;
  description: string;
  mime_type: string | null;
}

export interface McpContextPromptArgument {
  name: string;
  description: string;
  required: boolean;
}

export interface McpContextPrompt {
  server_id: string;
  name: string;
  description: string;
  arguments: McpContextPromptArgument[];
}

export interface McpContextSources {
  resources: McpContextResource[];
  prompts: McpContextPrompt[];
}

export type McpContextSelection =
  | {
      kind: "resource";
      server_id: string;
      uri: string;
      name: string;
      description?: string;
      mime_type?: string | null;
    }
  | {
      kind: "prompt";
      server_id: string;
      name: string;
      description?: string;
      arguments?: Record<string, string>;
    };

export async function createSession(
  workingDir: string,
  provider: string,
  model: string,
  apiKey = "",
): Promise<SessionCreated> {
  rememberWorkingDir(workingDir);

  try {
    return await invoke<SessionCreated>("create_session", {
      workingDir,
      provider,
      apiKey: apiKey || "",
      model,
    });
  } catch (error) {
    if (!isMissingTauriRuntimeError(error)) throw error;
    rememberWorkingDir(workingDir);
    return {
      session_id: `browser-${crypto.randomUUID()}`,
    };
  }
}

export async function resumeSession(sessionId: string): Promise<SessionCreated> {
  return invoke<SessionCreated>("resume_session", { sessionId });
}

export async function sendInput(
  sessionId: string,
  text: string,
  mcpContext: McpContextSelection[] = [],
): Promise<void> {
  return invoke("send_input", { sessionId, text, mcpContext });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function deleteSession(sessionId: string): Promise<void> {
  return invoke("delete_session", { sessionId });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

export async function listMcpContextSources(sessionId?: string): Promise<McpContextSources> {
  if (!hasTauriRuntime()) return { resources: [], prompts: [] };
  return invoke("list_mcp_context_sources", { sessionId: sessionId ?? null });
}

export async function getDefaultWorkingDir(): Promise<string> {
  try {
    return await invoke("get_default_working_dir");
  } catch (error) {
    if (!isMissingTauriRuntimeError(error)) throw error;
    return getRememberedWorkingDir() ?? "";
  }
}

export async function pickWorkspaceFolder(): Promise<string | null> {
  const mockPicker = (window as unknown as {
    __mockDirectoryPicker?: () => string | null | Promise<string | null>;
  }).__mockDirectoryPicker;
  if (mockPicker) return await mockPicker();
  if (!hasTauriRuntime()) return null;

  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择项目文件夹",
  });
  if (Array.isArray(selected)) return selected[0] ?? null;
  return selected ?? null;
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
  if (!hasTauriRuntime()) return [];
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

export async function getProjectRuntimeStatus(sessionId?: string): Promise<ProjectRuntimeStatus> {
  if (!hasTauriRuntime()) return fallbackProjectRuntimeStatus();
  return invoke("get_project_runtime_status", { sessionId: sessionId ?? null });
}

export async function startProjectDevServer(sessionId?: string): Promise<ProjectRuntimeStatus> {
  return invoke("start_project_dev_server", { sessionId: sessionId ?? null });
}

export async function stopProjectDevServer(sessionId?: string): Promise<ProjectRuntimeStatus> {
  return invoke("stop_project_dev_server", { sessionId: sessionId ?? null });
}

export async function openProjectPreview(sessionId?: string): Promise<ProjectRuntimeStatus> {
  return invoke("open_project_preview", { sessionId: sessionId ?? null });
}

export async function getProjectCheckpointStatus(sessionId?: string): Promise<ProjectCheckpointStatus> {
  if (!hasTauriRuntime()) return fallbackProjectCheckpointStatus();
  return invoke("get_project_checkpoint_status", { sessionId: sessionId ?? null });
}

export async function createProjectCheckpoint(sessionId?: string): Promise<ProjectCheckpointStatus> {
  return invoke("create_project_checkpoint", { sessionId: sessionId ?? null });
}

export async function restoreProjectCheckpoint(sessionId?: string): Promise<ProjectCheckpointStatus> {
  return invoke("restore_project_checkpoint", { sessionId: sessionId ?? null });
}

export async function listMemories(scope?: MemoryScope, projectPath?: string): Promise<WikiMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_memories", { scope: scope ?? null, projectPath: projectPath ?? null });
}

export async function updateMemory(
  memoryId: string,
  patch: MemoryPatch,
  sessionId?: string,
): Promise<WikiMemory> {
  return invoke("update_memory", { memoryId, patch, sessionId: sessionId ?? null });
}

export async function forgetMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("forget_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function pinMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("pin_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function selectContextMemories(
  message: string,
  projectPath?: string,
): Promise<SelectedContextMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("select_context_memories", { message, projectPath: projectPath ?? null });
}

export async function getWorkflowState(sessionId: string): Promise<WorkflowState | null> {
  if (!hasTauriRuntime()) return null;
  return invoke("get_workflow_state", { sessionId });
}

export async function overrideWorkflowRoute(
  sessionId: string,
  action: WorkflowOverrideAction,
): Promise<WorkflowState> {
  return invoke("override_workflow_route", { sessionId, action });
}

export async function getForgeWikiState(projectPath: string): Promise<ForgeWikiState> {
  if (!hasTauriRuntime()) return fallbackForgeWikiState(projectPath);
  return invoke("get_forge_wiki_state", { projectPath });
}

export async function initForgeWiki(projectPath: string): Promise<ForgeWikiState> {
  if (!hasTauriRuntime()) return fallbackForgeWikiState(projectPath);
  return invoke("init_forge_wiki", { projectPath });
}

export async function listForgeWikiPages(projectPath: string): Promise<ForgeWikiPage[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_forge_wiki_pages", { projectPath });
}

export async function readForgeWikiPage(projectPath: string, pagePath: string): Promise<string> {
  if (!hasTauriRuntime()) return "";
  return invoke("read_forge_wiki_page", { projectPath, pagePath });
}

export async function selectForgeWikiContext(
  projectPath: string,
  message: string,
): Promise<SelectedForgeWikiPage[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("select_forge_wiki_context", { projectPath, message });
}

export async function createForgeWikiUpdateProposal(
  projectPath: string,
  sessionId: string | null,
  targetPages: string[],
  title: string,
  summary: string,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("create_forge_wiki_update_proposal", {
    projectPath,
    sessionId,
    targetPages,
    title,
    summary,
  });
}

export async function acceptForgeWikiUpdateProposal(
  projectPath: string,
  proposalId: string,
  _sessionId?: string | null,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("accept_forge_wiki_update_proposal", { projectPath, proposalId });
}

export async function discardForgeWikiUpdateProposal(
  projectPath: string,
  proposalId: string,
  _sessionId?: string | null,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("discard_forge_wiki_update_proposal", { projectPath, proposalId });
}

export function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

function isMissingTauriRuntimeError(error: unknown): boolean {
  if (hasTauriRuntime()) return false;

  const message = String(error instanceof Error ? error.message : error);
  return [
    "__TAURI",
    "Tauri",
    "IPC",
    "invoke",
    "undefined",
  ].some((needle) => message.includes(needle));
}

function fallbackProjectRuntimeStatus(): ProjectRuntimeStatus {
  return {
    working_dir: getRememberedWorkingDir() ?? "",
    has_package_json: false,
    package_manager: "npm",
    dev_script: null,
    command: null,
    port: 1420,
    url: "http://localhost:1420",
    running: false,
    managed: false,
    pid: null,
    can_start: false,
    can_stop: false,
    can_open: false,
    message: "在桌面应用中读取交付状态",
    logs: [],
  };
}

function fallbackProjectCheckpointStatus(): ProjectCheckpointStatus {
  return {
    working_dir: getRememberedWorkingDir() ?? "",
    is_git_repo: false,
    dirty: false,
    last_checkpoint: null,
    message: "在桌面应用中读取检查点",
  };
}

function fallbackForgeWikiState(projectPath: string): ForgeWikiState {
  const normalizedProjectPath = projectPath || getRememberedWorkingDir() || "";
  return {
    project_path: normalizedProjectPath,
    exists: false,
    wiki_dir: normalizedProjectPath ? joinPath(normalizedProjectPath, ".forge", "wiki") : "",
    pages: [],
    message: "项目记录在浏览器预览中不可用。",
  };
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
