import { useCallback } from "react";
import { useStore } from "../store";
import { createSession, resumeSession, sendInput, killSession } from "../lib/tauri";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);
  const updateSessionStatus = useStore((s) => s.updateSessionStatus);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const create = useCallback(
    async (workingDir: string, provider = selectedProvider, model = selectedModel) => {
      try {
        const result = await createSession(workingDir, provider, model);
        addSession(result.session_id, provider, model);
        return result.session_id;
      } catch (e) {
        console.error("Failed to create session:", e);
        throw e;
      }
    },
    [addSession, selectedModel, selectedProvider]
  );

  const resume = useCallback(
    async (sessionId: string) => {
      try {
        const result = await resumeSession(sessionId);
        updateSessionStatus(result.session_id, "running");
        return result.session_id;
      } catch (e) {
        console.error("Failed to resume session:", e);
        throw e;
      }
    },
    [updateSessionStatus]
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
