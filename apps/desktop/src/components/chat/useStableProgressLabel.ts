import { useEffect, useState } from "react";
import {
  createStableProgressState,
  flushStableProgress,
  updateStableProgress,
  type LiveProgressCandidate,
} from "./conversationProgress.ts";

export function useStableProgressLabel(candidate: LiveProgressCandidate | null) {
  const [state, setState] = useState(() => createStableProgressState(candidate, progressNow()));

  useEffect(() => {
    setState((current) => updateStableProgress(
      current,
      candidate,
      progressNow(),
      candidate?.urgent === true,
    ));
  }, [candidate?.id, candidate?.label, candidate?.motion, candidate?.urgent]);

  useEffect(() => {
    if (state.dueAt === null || !state.pending) return;
    const delay = Math.max(0, state.dueAt - progressNow());
    const timer = window.setTimeout(() => {
      setState((current) => flushStableProgress(current, progressNow()));
    }, delay);
    return () => window.clearTimeout(timer);
  }, [state.dueAt, state.pending]);

  return state.visible;
}

function progressNow() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}
