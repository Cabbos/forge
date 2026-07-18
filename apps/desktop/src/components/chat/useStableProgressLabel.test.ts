import assert from "node:assert/strict";
import test from "node:test";
import * as progress from "./conversationProgress.ts";
import * as stableHook from "./useStableProgressLabel.ts";
import type {
  LiveProgressCandidate,
  StableProgressState,
} from "./conversationProgress.ts";

type StableProgressModule = {
  createStableProgressState?: (candidate: LiveProgressCandidate | null, now: number) => StableProgressState;
  updateStableProgress?: (
    state: StableProgressState,
    candidate: LiveProgressCandidate | null,
    now: number,
    urgent?: boolean,
  ) => StableProgressState;
  flushStableProgress?: (state: StableProgressState, now: number) => StableProgressState;
};

interface StableProgressTimerDriver {
  now: () => number;
  setTimer: (callback: () => void, delay: number) => number;
  clearTimer: (timer: number) => void;
}

type StableHookModule = {
  scheduleStableProgressFlush?: (
    dueAt: number,
    onDue: (now: number) => void,
    driver: StableProgressTimerDriver,
  ) => () => void;
};

test("delays the first stage for 240ms and coalesces to the latest candidate", () => {
  const module = stableModule();
  const analyzing = candidate("analyzing", "正在分析");
  const discovering = candidate("discovering", "正在查找相关内容");
  const initial = module.createStableProgressState(analyzing, 0);
  const coalesced = module.updateStableProgress(initial, discovering, 120);
  const early = module.flushStableProgress(coalesced, 239);
  const released = module.flushStableProgress(early, 240);

  assert.deepEqual(initial, {
    visible: null,
    visibleSince: 0,
    pending: analyzing,
    dueAt: 240,
    hasPresented: false,
  });
  assert.equal(coalesced.visible, null);
  assert.equal(coalesced.pending?.id, "discovering");
  assert.equal(coalesced.dueAt, 240);
  assert.strictEqual(early, coalesced);
  assert.deepEqual(released, {
    visible: discovering,
    visibleSince: 240,
    pending: null,
    dueAt: null,
    hasPresented: true,
  });
});

test("suppresses a fast answer before any progress was presented", () => {
  const module = stableModule();
  const initial = module.createStableProgressState(candidate("analyzing", "正在分析"), 0);
  const answered = module.updateStableProgress(
    initial,
    candidate("answering", "正在生成答复"),
    100,
  );

  assert.deepEqual(answered, emptyState(100));
});

test("shows urgent waiting immediately and pauses its motion", () => {
  const waiting = candidate("waiting", "等待你的确认", "paused", true);
  const state = stableModule().createStableProgressState(waiting, 12);

  assert.deepEqual(state, {
    visible: waiting,
    visibleSince: 12,
    pending: null,
    dueAt: null,
    hasPresented: true,
  });
});

test("leaves visible waiting immediately when active work resumes", () => {
  const module = stableModule();
  const waiting = candidate("waiting", "等待你的确认", "paused", true);
  const initial = module.createStableProgressState(waiting, 12);
  const refreshedWaiting = module.updateStableProgress(initial, { ...waiting }, 80, true);
  const resumed = module.updateStableProgress(
    refreshedWaiting,
    candidate("analyzing", "正在分析"),
    100,
  );

  assert.equal(refreshedWaiting.visible?.id, "waiting");
  assert.equal(refreshedWaiting.visibleSince, 12);
  assert.deepEqual(resumed, {
    visible: candidate("analyzing", "正在分析"),
    visibleSince: 100,
    pending: null,
    dueAt: null,
    hasPresented: true,
  });
});

test("holds a presented label for 600ms and keeps only the latest pending stage", () => {
  const module = stableModule();
  const analyzing = candidate("analyzing", "正在分析");
  const modifying = candidate("modifying", "正在进行修改");
  const verifying = candidate("verifying", "正在验证结果");
  const presented = module.flushStableProgress(module.createStableProgressState(analyzing, 0), 240);
  const queuedModify = module.updateStableProgress(presented, modifying, 420);
  const coalescedVerify = module.updateStableProgress(queuedModify, verifying, 700);
  const early = module.flushStableProgress(coalescedVerify, 839);
  const released = module.flushStableProgress(early, 840);

  assert.equal(queuedModify.visible?.id, "analyzing");
  assert.equal(queuedModify.pending?.id, "modifying");
  assert.equal(queuedModify.dueAt, 840);
  assert.equal(coalescedVerify.pending?.id, "verifying");
  assert.equal(coalescedVerify.dueAt, 840);
  assert.strictEqual(early, coalescedVerify);
  assert.equal(released.visible?.id, "verifying");
  assert.equal(released.visibleSince, 840);
  assert.equal(released.pending, null);
});

test("refreshes the visible candidate with the same id without another delay", () => {
  const module = stableModule();
  const analyzing = candidate("analyzing", "正在分析");
  const presented = module.flushStableProgress(module.createStableProgressState(analyzing, 0), 240);
  const refreshedCandidate = { ...analyzing };
  const refreshed = module.updateStableProgress(presented, refreshedCandidate, 300);

  assert.strictEqual(refreshed.visible, refreshedCandidate);
  assert.equal(refreshed.visibleSince, 240);
  assert.equal(refreshed.pending, null);
  assert.equal(refreshed.dueAt, null);
  assert.equal(refreshed.hasPresented, true);
});

test("does not flush before dueAt and clears state honestly for null", () => {
  const module = stableModule();
  const initial = module.createStableProgressState(candidate("discovering", "正在查找相关内容"), 10);
  const early = module.flushStableProgress(initial, 249);
  const cleared = module.updateStableProgress(initial, null, 100);

  assert.strictEqual(early, initial);
  assert.deepEqual(cleared, emptyState(100));
});

test("keeps presented progress visible across a tool-result gap into the answer stream", () => {
  const module = stableModule();
  const discovering = candidate("discovering", "正在查找相关内容");
  const answering = candidate("answering", "正在生成答复");
  const presented = module.flushStableProgress(
    module.createStableProgressState(discovering, 0),
    240,
  );
  const toolResultGap = module.updateStableProgress(presented, null, 300);
  const answerQueued = module.updateStableProgress(toolResultGap, answering, 320);
  const answerVisible = module.flushStableProgress(answerQueued, 840);

  assert.equal(toolResultGap.visible?.id, "discovering");
  assert.equal(toolResultGap.hasPresented, true);
  assert.equal(answerQueued.visible?.id, "discovering");
  assert.equal(answerQueued.pending?.id, "answering");
  assert.equal(answerQueued.dueAt, 840);
  assert.equal(answerVisible.visible?.id, "answering");
  assert.equal(answerVisible.pending, null);
});

test("clears resolved waiting progress immediately from visible or pending state", () => {
  const module = stableModule();
  const waiting = candidate("waiting", "等待你的确认", "paused", true);
  const visibleWaiting = module.createStableProgressState(waiting, 12);
  const pendingWaiting: StableProgressState = {
    visible: candidate("analyzing", "正在分析"),
    visibleSince: 0,
    pending: waiting,
    dueAt: 600,
    hasPresented: true,
  };

  assert.deepEqual(module.updateStableProgress(visibleWaiting, null, 80), emptyState(80));
  assert.deepEqual(module.updateStableProgress(pendingWaiting, null, 90), emptyState(90));
});

test("scheduler rounds up, reschedules an early wakeup, and flushes exactly once", () => {
  const schedule = (stableHook as StableHookModule).scheduleStableProgressFlush;
  assert.equal(typeof schedule, "function");
  let now = 239.25;
  let nextTimer = 0;
  const timers = new Map<number, () => void>();
  const delays: number[] = [];
  const cleared: number[] = [];
  const flushedAt: number[] = [];
  const driver: StableProgressTimerDriver = {
    now: () => now,
    setTimer: (callback, delay) => {
      const timer = ++nextTimer;
      timers.set(timer, callback);
      delays.push(delay);
      return timer;
    },
    clearTimer: (timer) => {
      cleared.push(timer);
      timers.delete(timer);
    },
  };
  const cancel = schedule!(240, (flushedNow) => flushedAt.push(flushedNow), driver);

  assert.deepEqual(delays, [1]);
  now = 239.75;
  timers.get(1)!();
  assert.deepEqual(delays, [1, 1]);
  assert.deepEqual(flushedAt, []);

  now = 240;
  timers.get(2)!();
  timers.get(2)!();
  assert.deepEqual(flushedAt, [240]);

  cancel();
  assert.deepEqual(cleared, []);
});

test("scheduler cleanup cancels a pending wakeup", () => {
  const schedule = (stableHook as StableHookModule).scheduleStableProgressFlush;
  assert.equal(typeof schedule, "function");
  let callback: (() => void) | null = null;
  const cleared: number[] = [];
  const flushedAt: number[] = [];
  const cancel = schedule!(10, (now) => flushedAt.push(now), {
    now: () => 0,
    setTimer: (nextCallback) => {
      callback = nextCallback;
      return 7;
    },
    clearTimer: (timer) => cleared.push(timer),
  });

  cancel();
  (callback as (() => void) | null)?.();

  assert.deepEqual(cleared, [7]);
  assert.deepEqual(flushedAt, []);
});

function stableModule() {
  const module = progress as StableProgressModule;
  assert.equal(typeof module.createStableProgressState, "function");
  assert.equal(typeof module.updateStableProgress, "function");
  assert.equal(typeof module.flushStableProgress, "function");
  return module as Required<StableProgressModule>;
}

function candidate(
  id: LiveProgressCandidate["id"],
  label: string,
  motion: "live" | "paused" = "live",
  urgent?: boolean,
): LiveProgressCandidate {
  return urgent ? { id, label, motion, urgent } : { id, label, motion };
}

function emptyState(now: number): StableProgressState {
  return {
    visible: null,
    visibleSince: now,
    pending: null,
    dueAt: null,
    hasPresented: false,
  };
}
