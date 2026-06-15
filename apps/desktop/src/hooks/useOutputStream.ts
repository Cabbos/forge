import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { hasTauriRuntime } from "../lib/tauri";
import { useStore } from "../store";
import type { StreamEvent } from "../lib/protocol";
import { shouldSubscribeToTauriOutputStream } from "./outputStreamRuntime";

export function useOutputStream(sessionId: string | null) {
  useEffect(() => {
    if (!shouldSubscribeToTauriOutputStream(hasTauriRuntime())) return;

    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    const setup = async () => {
      const cleanup = await listen<StreamEvent>("session-output", (event) => {
        if (
          event.payload.event_type === "recovery_notice" ||
          event.payload.event_type === "health_alert"
        ) {
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
