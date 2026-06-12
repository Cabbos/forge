import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useStore } from "../store";
import type { StreamEvent } from "../lib/protocol";

export function useOutputStream(sessionId: string | null) {
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    const setup = async () => {
      const cleanup = await listen<StreamEvent>("session-output", (event) => {
        if (event.payload.event_type === "recovery_notice") {
          useStore.getState().dispatchOutputEvent(event.payload);
          return;
        }
        if (!sessionId) return;
        if (event.payload.session_id !== sessionId) return;
        useStore.getState().dispatchOutputEvent(event.payload);
      });

      if (disposed) {
        cleanup();
        return;
      }

      unlisten = cleanup;
    };

    setup();

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [sessionId]);
}
