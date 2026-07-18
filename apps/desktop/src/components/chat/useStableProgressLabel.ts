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
    return scheduleStableProgressFlush(
      state.dueAt,
      (now) => setState((current) => flushStableProgress(current, now)),
      {
        now: progressNow,
        setTimer: (callback, delay) => window.setTimeout(callback, delay),
        clearTimer: (timer) => window.clearTimeout(timer),
      },
    );
  }, [state.dueAt, state.pending]);

  return state.visible;
}

export interface StableProgressTimerDriver {
  now: () => number;
  setTimer: (callback: () => void, delay: number) => number;
  clearTimer: (timer: number) => void;
}

export function scheduleStableProgressFlush(
  dueAt: number,
  onDue: (now: number) => void,
  driver: StableProgressTimerDriver,
) {
  let timer: number | null = null;
  let cancelled = false;
  let completed = false;

  const schedule = () => {
    const delay = Math.max(0, Math.ceil(dueAt - driver.now()));
    timer = driver.setTimer(() => {
      timer = null;
      if (cancelled || completed) return;

      const now = driver.now();
      if (now < dueAt) {
        schedule();
        return;
      }

      completed = true;
      onDue(now);
    }, delay);
  };

  schedule();

  return () => {
    cancelled = true;
    if (timer !== null) {
      driver.clearTimer(timer);
      timer = null;
    }
  };
}

function progressNow() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}
