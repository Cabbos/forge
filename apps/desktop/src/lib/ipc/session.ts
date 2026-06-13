import { hasTauriRuntime } from "./core";
import type {
  ComposerCapabilitySelection,
  ManualCompactResult,
  McpContextSelection,
  SessionCreated,
  StreamEvent,
} from "./types";
import { rememberWorkingDir } from "./app";

export async function createSession(
  workingDir: string,
  provider: string,
  model: string,
  apiKey = "",
): Promise<SessionCreated> {
  if (!hasTauriRuntime()) {
    rememberWorkingDir(workingDir);
    return { session_id: `browser-${crypto.randomUUID()}` };
  }

  const { invoke } = await import("@tauri-apps/api/core");
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
    return { session_id: `browser-${crypto.randomUUID()}` };
  }
}

export async function resumeSession(sessionId: string): Promise<SessionCreated> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<SessionCreated>("resume_session", { sessionId });
}

export async function sendInput(
  sessionId: string,
  text: string,
  mcpContext: McpContextSelection[] = [],
  capabilities: ComposerCapabilitySelection[] = [],
): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("send_input", { sessionId, text, mcpContext, capabilities });
}

export async function compactSessionContext(sessionId: string): Promise<ManualCompactResult> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<ManualCompactResult>("compact_session_context", { sessionId });
}

export async function killSession(sessionId: string): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("kill_session", { sessionId });
}

export async function deleteSession(sessionId: string): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("delete_session", { sessionId });
}

export async function loadSessionTranscript(sessionId: string): Promise<StreamEvent[]> {
  if (!hasTauriRuntime()) return [];
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<StreamEvent[]>("load_session_transcript", { sessionId });
}

export async function confirmResponse(blockId: string, approved: boolean): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("confirm_response", { blockId, approved });
}

function isMissingTauriRuntimeError(error: unknown): boolean {
  if (hasTauriRuntime()) return false;
  const message = String(error instanceof Error ? error.message : error);
  return ["__TAURI", "Tauri", "IPC", "invoke", "undefined"].some((needle) =>
    message.includes(needle)
  );
}
