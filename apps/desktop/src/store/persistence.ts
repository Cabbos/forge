import { get as idbGet, set as idbSet, del as idbDel } from "idb-keyval";
import type { BlockState, DeliverySummary, StreamEvent, WorkflowState } from "../lib/protocol";
import type { ProviderId } from "../lib/providers";
import { hasTauriRuntime, loadSessionTranscript, saveAppMetadata } from "../lib/tauri";
import type { Workspace } from "../lib/workspaces";
import { sortWorkspaces } from "../lib/workspaces";
import type { PersistedSession } from "./types";

export const PERSIST_KEY = "forge-sessions";
export const BLOCKS_PREFIX = "forge-blocks:";
export const PROVIDER_KEY = "forge-provider";
export const MODEL_KEY = "forge-model";
export const ACTIVE_SESSION_KEY = "forge-active-session";
export const WORKSPACES_KEY = "forge-workspaces";
export const ACTIVE_WORKSPACE_KEY = "forge-active-workspace";
export const LEGACY_WORKING_DIR_KEY = "forge-working-dir";

const MAX_PERSISTED_BLOCKS = 100;
const BLOCK_PERSIST_DEBOUNCE_MS = 350;
const blockPersistTimers = new Map<string, ReturnType<typeof setTimeout>>();

export function persistWorkspaces(workspaces: Map<string, Workspace>, activeWorkspaceId: string | null) {
  if (hasTauriRuntime()) return Promise.resolve([]);
  return Promise.all([
    idbSet(WORKSPACES_KEY, sortWorkspaces(workspaces.values())).catch(() => {}),
    activeWorkspaceId
      ? idbSet(ACTIVE_WORKSPACE_KEY, activeWorkspaceId).catch(() => {})
      : idbDel(ACTIVE_WORKSPACE_KEY).catch(() => {}),
  ]);
}

export function persistSessions(
  sessions: Map<string, PersistableSession>,
  workflowBySession: Map<string, WorkflowState>,
  deliverySummaryBySession: Map<string, DeliverySummary>,
) {
  if (hasTauriRuntime()) return Promise.resolve();
  const data: PersistedSession[] = [];
  sessions.forEach((s) => {
    data.push({
      id: s.id,
      agentType: s.agentType,
      model: s.model,
      workingDir: s.workingDir ?? null,
      workspaceId: s.workspaceId ?? null,
      createdAt: s.createdAt ?? null,
      updatedAt: s.updatedAt ?? null,
      costUsd: s.costUsd ?? null,
      contextWindowTokens: s.contextWindowTokens ?? null,
      contextUsage: s.contextUsage ?? null,
      usageLedger: s.usageLedger ?? null,
      status: s.status,
      workflowState: workflowBySession.get(s.id) ?? null,
      deliverySummary: deliverySummaryBySession.get(s.id) ?? null,
    });
  });
  return idbSet(PERSIST_KEY, data).catch(() => {});
}

export function persistBackendAppMetadata(snapshot: {
  workspaces: Map<string, Workspace>;
  activeWorkspaceId: string | null;
  activeSessionId: string | null;
  selectedProvider: ProviderId;
  selectedModel: string;
}) {
  if (!hasTauriRuntime()) return Promise.resolve();
  return saveAppMetadata({
    workspaces: sortWorkspaces(snapshot.workspaces.values()).map((workspace) => ({
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
      lastOpenedAt: workspace.lastOpenedAt,
    })),
    activeWorkspaceId: snapshot.activeWorkspaceId,
    activeSessionId: snapshot.activeSessionId,
    selectedProvider: snapshot.selectedProvider,
    selectedModel: snapshot.selectedModel,
  }).catch((error) => {
    console.warn("[app-metadata] failed to persist metadata", error);
  });
}

function cappedBlocks(blocks: BlockState[]) {
  return blocks.length > MAX_PERSISTED_BLOCKS
    ? blocks.slice(blocks.length - MAX_PERSISTED_BLOCKS)
    : blocks;
}

export function clearPendingBlockPersist(sessionId: string) {
  const timer = blockPersistTimers.get(sessionId);
  if (timer) {
    clearTimeout(timer);
    blockPersistTimers.delete(sessionId);
  }
}

export function persistBlocks(sessionId: string, blocks: BlockState[]) {
  if (hasTauriRuntime()) return;
  const snapshot = cappedBlocks(blocks);
  clearPendingBlockPersist(sessionId);
  blockPersistTimers.set(sessionId, setTimeout(() => {
    blockPersistTimers.delete(sessionId);
    idbSet(BLOCKS_PREFIX + sessionId, snapshot).catch(() => {});
  }, BLOCK_PERSIST_DEBOUNCE_MS));
}

export function persistBlocksNow(sessionId: string, blocks: BlockState[]) {
  clearPendingBlockPersist(sessionId);
  if (hasTauriRuntime()) return Promise.resolve();
  return idbSet(BLOCKS_PREFIX + sessionId, cappedBlocks(blocks)).catch(() => {});
}

export async function loadBlocks(
  sessionId: string,
  transcriptEventsToBlocks: (events: StreamEvent[]) => BlockState[],
): Promise<BlockState[]> {
  try {
    if (hasTauriRuntime()) {
      const transcriptEvents = await loadSessionTranscript(sessionId).catch(() => []);
      if (transcriptEvents.length > 0) return transcriptEventsToBlocks(transcriptEvents);
    }
    const blocks = await idbGet<BlockState[]>(BLOCKS_PREFIX + sessionId);
    return blocks ?? [];
  } catch {
    return [];
  }
}

type PersistableSession = {
  id: string;
  agentType: string;
  model: string;
  workingDir?: string | null;
  workspaceId?: string | null;
  createdAt?: number | null;
  updatedAt?: number | null;
  costUsd?: number | null;
  contextWindowTokens?: number | null;
  contextUsage?: PersistedSession["contextUsage"];
  usageLedger?: PersistedSession["usageLedger"];
  status: PersistedSession["status"];
};
