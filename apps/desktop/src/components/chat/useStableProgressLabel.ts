import { useEffect, useState } from "react";
import type { LiveProgressCandidate } from "./conversationTurnView.ts";
import {
  createStableProgressState,
  flushStableProgress,
  updateStableProgress,
} from "./conversationProgress.ts";

export function useStableProgressLabel(candidate: LiveProgressCandidate | null) {
  const [state, setState] = useState(() => createStableProgressState(candidate, progressNow()));

  useEffect(() => {
    setState((current) => updateStableProgress(current, candidate, progressNow()));
  }, [candidate?.id, candidate?.label]);

  useEffect(() => {
    if (state.dueAt === null || !state.pending) return;
    const delay = Math.max(0, state.dueAt - progressNow());
    const timer = window.setTimeout(() => {
      setState((current) => flushStableProgress(current, progressNow()));
    }, delay);
    return () => window.clearTimeout(timer);
  }, [state.dueAt, state.pending?.id, state.pending?.label]);

  return state.visible;
}

function progressNow() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}
