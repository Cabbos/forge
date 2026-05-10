import { useCallback } from "react";
import { useStore } from "../store";
import {
  createSession,
  sendInput,
  sendSignal,
  killSession,
} from "../lib/tauri";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);

  const create = useCallback(
    async (toolType: string, workingDir: string, toolPath?: string, model?: string) => {
      try {
        const result = await createSession(toolType, workingDir, toolPath, model);
        addSession(result.session_id, toolType, model || "");
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

  const interrupt = useCallback(async (sessionId: string) => {
    try {
      await sendSignal(sessionId, "interrupt");
    } catch (e) {
      console.error("Failed to send interrupt:", e);
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

  return { create, send, interrupt, kill };
}
