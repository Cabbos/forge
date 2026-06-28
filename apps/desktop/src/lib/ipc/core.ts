import { invoke } from "@tauri-apps/api/core";

export function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

export function isMissingTauriRuntimeError(error: unknown): boolean {
  if (hasTauriRuntime()) return false;

  const message = String(error instanceof Error ? error.message : error);
  return ["__TAURI", "Tauri", "IPC", "invoke", "undefined"].some((needle) =>
    message.includes(needle)
  );
}

export async function tauriInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return invoke<T>(command, args);
}
