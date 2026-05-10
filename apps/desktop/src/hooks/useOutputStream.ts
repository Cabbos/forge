import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useStore } from "../store";
import type { StreamEvent } from "../lib/protocol";

export function useOutputStream(sessionId: string | null) {
  useEffect(() => {
    if (!sessionId) return;

    let unlisten: UnlistenFn | null = null;

    const setup = async () => {
      unlisten = await listen<StreamEvent>("session-output", (event) => {
        if (event.payload.session_id !== sessionId) return;
        useStore.getState().dispatchOutputEvent(event.payload);
      });
    };

    setup();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [sessionId]);
}
