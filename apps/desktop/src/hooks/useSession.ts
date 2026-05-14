import { useCallback } from "react";
import { useStore } from "../store";
import { createSession, resumeSession, sendInput, killSession } from "../lib/tauri";
import { getProviderLabel } from "../lib/providers";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);
  const updateSessionStatus = useStore((s) => s.updateSessionStatus);
  const dispatchOutputEvent = useStore((s) => s.dispatchOutputEvent);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const create = useCallback(
    async (workingDir: string, provider = selectedProvider, model = selectedModel) => {
      try {
        const result = await createSession(workingDir, provider, model);
        addSession(result.session_id, result.provider ?? provider, result.model ?? model, workingDir);
        if (result.missing_api_key) {
          const providerLabel = getProviderLabel(result.provider ?? provider);
          dispatchOutputEvent({
            event_type: "error",
            session_id: result.session_id,
            block_id: crypto.randomUUID(),
            message: `还没有配置 ${providerLabel} API Key。请打开设置，粘贴密钥后就可以开始发送。`,
            code: "missing_api_key",
          });
        }
        return result.session_id;
      } catch (e) {
        console.error("Failed to create session:", e);
        throw e;
      }
    },
    [addSession, dispatchOutputEvent, selectedModel, selectedProvider]
  );

  const resume = useCallback(
    async (sessionId: string) => {
      try {
        const result = await resumeSession(sessionId);
        updateSessionStatus(result.session_id, "running");
        if (result.missing_api_key) {
          const providerLabel = getProviderLabel(result.provider);
          dispatchOutputEvent({
            event_type: "error",
            session_id: result.session_id,
            block_id: crypto.randomUUID(),
            message: `还没有配置 ${providerLabel} API Key。请打开设置，粘贴密钥后就可以开始发送。`,
            code: "missing_api_key",
          });
        }
        return result.session_id;
      } catch (e) {
        console.error("Failed to resume session:", e);
        throw e;
      }
    },
    [dispatchOutputEvent, updateSessionStatus]
  );

  const send = useCallback(async (sessionId: string, text: string) => {
    try {
      await sendInput(sessionId, text);
    } catch (e) {
      console.error("Failed to send input:", e);
    }
  }, []);

  const kill = useCallback(
    async (sessionId: string) => {
      try {
        await killSession(sessionId);
        removeSession(sessionId);
      } catch (e) {
        console.error("Failed to kill session:", e);
      }
    },
    [removeSession]
  );

  return { create, resume, send, kill };
}
