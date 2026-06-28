import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { hasTauriRuntime, isMissingTauriRuntimeError } from "./core";
import type { AppMetadata, SessionInfo } from "./types";

const WORKING_DIR_KEY = "forge-working-dir";

export function rememberWorkingDir(workingDir: string) {
  if (typeof window === "undefined" || !workingDir.trim()) return;
  window.localStorage.setItem(WORKING_DIR_KEY, workingDir.trim());
}

export function getRememberedWorkingDir(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(WORKING_DIR_KEY);
}

export async function loadAppMetadata(): Promise<AppMetadata> {
  if (!hasTauriRuntime()) {
    return {
      workspaces: [],
      activeWorkspaceId: null,
      activeSessionId: null,
      selectedProvider: null,
      selectedModel: null,
    };
  }
  return invoke("load_app_metadata");
}

export async function saveAppMetadata(metadata: AppMetadata): Promise<void> {
  if (!hasTauriRuntime()) return;
  return invoke("save_app_metadata", { metadata });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
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
