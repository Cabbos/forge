import { useCallback } from "react";
import { useStore } from "../store";
import { createSession, sendInput, killSession } from "../lib/tauri";

const DEFAULT_MODEL = "deepseek-v4-flash[1m]";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);

  const create = useCallback(
    async (workingDir: string, model?: string) => {
      try {
        const m = model || DEFAULT_MODEL;
        const result = await createSession(workingDir, "", m);
        addSession(result.session_id, "deepseek", m);
        return result.session_id;
      } catch (e) {
        console.error("Failed to create session:", e);
        throw e;
      }
    },
    [addSession]
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

  return { create, send, kill };
}
