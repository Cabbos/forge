import { create } from "zustand";
import { get as idbGet, set as idbSet, del as idbDel } from "idb-keyval";
import type {
  AgentTurnProjection,
  BlockState,
  ForgeWikiUpdateProposal,
  McpContextStatus,
  SelectedContextMemory,
  SelectedForgeWikiPage,
  StreamEvent,
  SessionState,
  WikiMemory,
  WorkflowState,
  DeliverySummary,
} from "../lib/protocol";
import type { FirstLoopDraft } from "../lib/first-loop";
import type { McpContextSelection, SessionInfo } from "../lib/tauri";
import type { Workspace } from "../lib/workspaces";
import {
  hasTauriRuntime,
  listSessions,
  loadAppMetadata,
  loadSessionTranscript,
  saveAppMetadata,
} from "../lib/tauri";
import {
  normalizeWorkspacePath,
  sortWorkspaces,
  workspaceFromPath,
} from "../lib/workspaces";
import {
  DEFAULT_PROVIDER_ID,
  getDefaultModel,
  getModelContextWindow,
  modelBelongsToProvider,
  normalizeProviderId,
  type ProviderId,
} from "../lib/providers";

interface AppStore {
  // Sessions
  sessions: Map<string, SessionState>;
  activeSessionId: string | null;
  hydrated: boolean;
  workspaces: Map<string, Workspace>;
  activeWorkspaceId: string | null;
  memories: WikiMemory[];
  selectedContextBySession: Map<string, SelectedContextMemory[]>;
  forgeWikiContextBySession: Map<string, SelectedForgeWikiPage[]>;
  mcpContextBySession: Map<string, McpContextSelection[]>;
  mcpContextStatusBySession: Map<string, Map<string, McpContextStatus>>;
  forgeWikiProposalsBySession: Map<string, ForgeWikiUpdateProposal[]>;
  workflowBySession: Map<string, WorkflowState>;
  agentTurnBySession: Map<string, AgentTurnProjection>;
  firstLoopDraftBySession: Map<string, FirstLoopDraft>;
  deliverySummaryBySession: Map<string, DeliverySummary>;

  // Provider
  selectedProvider: ProviderId;
  setSelectedProvider: (p: string) => void;
  selectedModel: string;
  setSelectedModel: (m: string) => void;

  // Actions
  hydrate: () => Promise<void>;
  setActiveSession: (id: string | null) => void;
  setActiveWorkspace: (id: string | null) => void;
  upsertWorkspace: (workspace: Workspace) => void;
  removeWorkspace: (id: string) => void;
  addSession: (id: string, provider: string, model: string, workingDir?: string | null) => void;
  removeSession: (id: string) => void;
  setMemories: (memories: WikiMemory[]) => void;
  upsertMemory: (memory: WikiMemory) => void;
  setForgeWikiContext: (sessionId: string, selected: SelectedForgeWikiPage[]) => void;
  toggleMcpContext: (sessionId: string, selection: McpContextSelection) => void;
  clearMcpContext: (sessionId: string) => void;
  upsertForgeWikiProposal: (sessionId: string, proposal: ForgeWikiUpdateProposal) => void;
  setWorkflowState: (sessionId: string, workflow: WorkflowState) => void;
  setFirstLoopDraft: (sessionId: string, draft: FirstLoopDraft | null) => void;
  updateSessionStatus: (id: string, status: SessionState["status"]) => void;
  updateBlock: (sessionId: string, blockId: string, patch: Partial<BlockState>) => void;

  // Output events
  dispatchOutputEvent: (event: StreamEvent) => void;
  addUserMessage: (sessionId: string, text: string) => void;

  // Input
  pendingInput: string;
  setPendingInput: (text: string) => void;

  // Theme
  theme: "light" | "dark";
  setTheme: (theme: "light" | "dark") => void;
}

const PERSIST_KEY = "forge-sessions";
const BLOCKS_PREFIX = "forge-blocks:";
const PROVIDER_KEY = "forge-provider";
const MODEL_KEY = "forge-model";
const ACTIVE_SESSION_KEY = "forge-active-session";
const WORKSPACES_KEY = "forge-workspaces";
const ACTIVE_WORKSPACE_KEY = "forge-active-workspace";
const LEGACY_WORKING_DIR_KEY = "forge-working-dir";
const MAX_PERSISTED_BLOCKS = 100;
const BLOCK_PERSIST_DEBOUNCE_MS = 350;
const blockPersistTimers = new Map<string, ReturnType<typeof setTimeout>>();

interface PersistedSession {
  id: string;
  agentType: string;
  model: string;
  workingDir?: string | null;
  workspaceId?: string | null;
  createdAt?: number | null;
  updatedAt?: number | null;
  contextWindowTokens?: number | null;
  status: SessionState["status"];
  workflowState?: WorkflowState | null;
  deliverySummary?: DeliverySummary | null;
}

function persistedSessionFromBackend(info: SessionInfo): PersistedSession {
  return {
    id: info.id,
    agentType: info.provider,
    model: info.model,
    workingDir: info.working_dir ?? null,
    workspaceId: info.working_dir ?? null,
    createdAt: info.created_at_ms ?? null,
    updatedAt: info.updated_at_ms ?? info.created_at_ms ?? null,
    contextWindowTokens: info.context_window_tokens ?? null,
    status: coerceSessionStatus(info.status),
    workflowState: info.latest_workflow ?? null,
    deliverySummary: info.latest_delivery ?? null,
  };
}

function coerceSessionStatus(status: string): SessionState["status"] {
  if (status === "running" || status === "error") return status;
  return "stopped";
}

function persistWorkspaces(workspaces: Map<string, Workspace>, activeWorkspaceId: string | null) {
  if (hasTauriRuntime()) return Promise.resolve([]);
  return Promise.all([
    idbSet(WORKSPACES_KEY, sortWorkspaces(workspaces.values())).catch(() => {}),
    activeWorkspaceId
      ? idbSet(ACTIVE_WORKSPACE_KEY, activeWorkspaceId).catch(() => {})
      : idbDel(ACTIVE_WORKSPACE_KEY).catch(() => {}),
  ]);
}

// Save sessions to IndexedDB. Returns a promise so callers can await when needed.
function persistSessions(
  sessions: Map<string, SessionState>,
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
      contextWindowTokens: s.contextWindowTokens ?? null,
      status: s.status,
      workflowState: workflowBySession.get(s.id) ?? null,
      deliverySummary: deliverySummaryBySession.get(s.id) ?? null,
    });
  });
  return idbSet(PERSIST_KEY, data).catch(() => {});
}

function persistBackendAppMetadata(snapshot: {
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

function clearPendingBlockPersist(sessionId: string) {
  const timer = blockPersistTimers.get(sessionId);
  if (timer) {
    clearTimeout(timer);
    blockPersistTimers.delete(sessionId);
  }
}

// Save blocks for a session to IndexedDB (capped at MAX_PERSISTED_BLOCKS).
// Streaming can produce dozens of chunks per second, so debounce disk writes.
function persistBlocks(sessionId: string, blocks: BlockState[]) {
  if (hasTauriRuntime()) return;
  const snapshot = cappedBlocks(blocks);
  clearPendingBlockPersist(sessionId);
  blockPersistTimers.set(sessionId, setTimeout(() => {
    blockPersistTimers.delete(sessionId);
    idbSet(BLOCKS_PREFIX + sessionId, snapshot).catch(() => {});
  }, BLOCK_PERSIST_DEBOUNCE_MS));
}

function persistBlocksNow(sessionId: string, blocks: BlockState[]) {
  clearPendingBlockPersist(sessionId);
  if (hasTauriRuntime()) return Promise.resolve();
  return idbSet(BLOCKS_PREFIX + sessionId, cappedBlocks(blocks)).catch(() => {});
}

// Load blocks for a session from IndexedDB
async function loadBlocks(sessionId: string): Promise<BlockState[]> {
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

function transcriptEventsToBlocks(events: StreamEvent[]): BlockState[] {
  let blocks: BlockState[] = [];
  for (const event of events) {
    blocks = applyTranscriptEventToBlocks(blocks, event);
  }
  return blocks.filter((block) => block.event_type !== "pending");
}

function applyTranscriptEventToBlocks(blocks: BlockState[], event: StreamEvent): BlockState[] {
  const event_type = event.event_type;

  if (event_type === "delivery_summary" && isSameAsLastDeliveryBlock(blocks, event.summary)) {
    return blocks;
  }

  if (event_type === "error") {
    return [
      ...blocks,
      {
        block_id: event.block_id,
        event_type: "error",
        content: event.message,
        metadata: { code: event.code },
        isComplete: true,
      },
    ];
  }

  if (event_type === "tool_call_result") {
    const next = [...blocks];
    let existingIdx = next.findIndex((block) =>
      (block.event_type === "tool_call" || block.event_type === "shell" || block.event_type === "thinking") &&
      block.block_id === event.block_id
    );
    if (existingIdx < 0) {
      existingIdx = [...next].reverse().findIndex((block) =>
        (block.event_type === "tool_call" || block.event_type === "shell" || block.event_type === "thinking") &&
        (!block.content || block.content === "")
      );
      if (existingIdx >= 0) existingIdx = next.length - 1 - existingIdx;
    }
    if (existingIdx >= 0) {
      next[existingIdx] = {
        ...next[existingIdx],
        content: event.result,
        isComplete: true,
        metadata: {
          ...next[existingIdx].metadata,
          is_error: event.is_error,
          duration_ms: event.duration_ms,
        },
      };
      return next;
    }
    return [
      ...next,
      {
        block_id: event.block_id,
        event_type: "tool_call",
        content: event.result,
        isComplete: true,
        metadata: {
          is_error: event.is_error,
          duration_ms: event.duration_ms,
          tool_name: "Tool",
        },
      },
    ];
  }

  if (event_type === "thinking_chunk" || event_type === "text_chunk" || event_type === "shell_output") {
    const next = [...blocks];
    const existingIdx = next.findIndex((block) => block.block_id === event.block_id);
    const blockType = event_type === "thinking_chunk" ? "thinking" : event_type === "shell_output" ? "shell" : "text";
    if (existingIdx >= 0) {
      next[existingIdx] = {
        ...next[existingIdx],
        content: next[existingIdx].content + event.content,
      };
      return next;
    }
    return [
      ...next,
      {
        block_id: event.block_id,
        event_type: blockType,
        content: event.content,
        isComplete: false,
        metadata: {},
      },
    ];
  }

  if (event_type === "thinking_end" || event_type === "text_end" || event_type === "shell_end" || event_type === "tool_call_end") {
    const next = [...blocks];
    const existingIdx = next.findIndex((block) => block.block_id === event.block_id);
    if (existingIdx >= 0) {
      if (event_type !== "tool_call_end") {
        next[existingIdx] = { ...next[existingIdx], isComplete: true };
      }
      if (event_type === "shell_end") {
        next[existingIdx] = {
          ...next[existingIdx],
          metadata: { ...next[existingIdx].metadata, exit_code: event.exit_code },
        };
      }
    }
    return next;
  }

  const block = eventToBlock(event);
  return block ? [...blocks, block] : blocks;
}

function latestDeliverySummaryFromBlocks(blocks: BlockState[]): DeliverySummary | null {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block?.event_type !== "delivery_summary") continue;
    return parsePersistedDeliverySummary(block.metadata?.summary);
  }
  return null;
}

function lastNonPendingBlock(blocks: BlockState[]): BlockState | null {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block?.event_type !== "pending") return block;
  }
  return null;
}

function isSameAsLastDeliveryBlock(blocks: BlockState[], summary: DeliverySummary): boolean {
  const lastBlock = lastNonPendingBlock(blocks);
  if (lastBlock?.event_type !== "delivery_summary") return false;
  return deliverySummariesEqual(parsePersistedDeliverySummary(lastBlock.metadata?.summary), summary);
}

function parsePersistedDeliverySummary(value: unknown): DeliverySummary | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Partial<Record<keyof DeliverySummary, unknown>>;
  const previewLabel = stringValue(record.preview_label);
  const checkpointLabel = stringValue(record.checkpoint_label);
  const nextAction = stringValue(record.next_action);
  if (!previewLabel || !checkpointLabel || !nextAction) return null;
  return {
    project_path: stringValue(record.project_path),
    preview_label: previewLabel,
    checkpoint_label: checkpointLabel,
    next_action: nextAction,
    verification_label: stringValue(record.verification_label),
    verification_status: stringValue(record.verification_status),
    verification_command: stringValue(record.verification_command),
    record_label: stringValue(record.record_label),
    record_status: stringValue(record.record_status),
    record_target_pages: Array.isArray(record.record_target_pages)
      ? record.record_target_pages.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
      : [],
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function getLegacyWorkspace(): Workspace | null {
  if (typeof window === "undefined") return null;
  return workspaceFromPath(window.localStorage.getItem(LEGACY_WORKING_DIR_KEY) ?? "");
}

function workspaceSessionIds(sessions: Map<string, SessionState>, workspaceId: string | null) {
  if (!workspaceId) {
    return Array.from(sessions.values())
      .filter((session) => !session.workspaceId && !session.workingDir)
      .map((session) => session.id);
  }
  return Array.from(sessions.values())
    .filter((session) => session.workspaceId === workspaceId || session.workingDir === workspaceId)
    .map((session) => session.id);
}

function sameMcpContextSelection(a: McpContextSelection, b: McpContextSelection) {
  if (a.kind !== b.kind || a.server_id !== b.server_id) return false;
  return a.kind === "resource" && b.kind === "resource"
    ? a.uri === b.uri
    : a.kind === "prompt" && b.kind === "prompt" && a.name === b.name;
}

export const useStore = create<AppStore>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  hydrated: false,
  workspaces: new Map(),
  activeWorkspaceId: null,
  memories: [],
  selectedContextBySession: new Map(),
  forgeWikiContextBySession: new Map(),
  mcpContextBySession: new Map(),
  mcpContextStatusBySession: new Map(),
  forgeWikiProposalsBySession: new Map(),
  workflowBySession: new Map(),
  agentTurnBySession: new Map(),
  firstLoopDraftBySession: new Map(),
  deliverySummaryBySession: new Map(),
  pendingInput: "",
  selectedProvider: DEFAULT_PROVIDER_ID,
  selectedModel: getDefaultModel(DEFAULT_PROVIDER_ID),

  setSelectedProvider: (p) => {
    const selectedProvider = normalizeProviderId(p);
    const currentModel = get().selectedModel;
    const selectedModel = modelBelongsToProvider(selectedProvider, currentModel)
      ? currentModel
      : getDefaultModel(selectedProvider);
    set({ selectedProvider, selectedModel });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces: get().workspaces,
        activeWorkspaceId: get().activeWorkspaceId,
        activeSessionId: get().activeSessionId,
        selectedProvider,
        selectedModel,
      });
    } else {
      idbSet(PROVIDER_KEY, selectedProvider).catch(() => {});
      idbSet(MODEL_KEY, selectedModel).catch(() => {});
    }
  },

  setSelectedModel: (m) => {
    set({ selectedModel: m });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces: get().workspaces,
        activeWorkspaceId: get().activeWorkspaceId,
        activeSessionId: get().activeSessionId,
        selectedProvider: get().selectedProvider,
        selectedModel: m,
      });
    } else {
      idbSet(MODEL_KEY, m).catch(() => {});
    }
  },

  hydrate: async () => {
    try {
      const tauriRuntime = hasTauriRuntime();
      const backendMetadata = tauriRuntime ? await loadAppMetadata().catch(() => null) : null;
      const backendSessions = tauriRuntime ? await listSessions().catch(() => []) : [];
      const data = tauriRuntime
        ? backendSessions.map(persistedSessionFromBackend)
        : await idbGet<PersistedSession[]>(PERSIST_KEY);
      const savedWorkspaces = tauriRuntime
        ? backendMetadata?.workspaces ?? null
        : await idbGet<Workspace[]>(WORKSPACES_KEY).catch(() => null);
      const savedActiveWorkspaceId = tauriRuntime
        ? backendMetadata?.activeWorkspaceId ?? null
        : await idbGet<string>(ACTIVE_WORKSPACE_KEY).catch(() => null);
      const savedTheme = await idbGet<string>("tui-theme").catch(() => null);
      const savedProvider = tauriRuntime
        ? backendMetadata?.selectedProvider ?? null
        : await idbGet<string>(PROVIDER_KEY).catch(() => null);
      const savedModel = tauriRuntime
        ? backendMetadata?.selectedModel ?? null
        : await idbGet<string>(MODEL_KEY).catch(() => null);
      const savedActiveSessionId = tauriRuntime
        ? backendMetadata?.activeSessionId ?? null
        : await idbGet<string>(ACTIVE_SESSION_KEY).catch(() => null);
      const selectedProvider = normalizeProviderId(savedProvider);
      const selectedModel = savedModel && modelBelongsToProvider(selectedProvider, savedModel)
        ? savedModel
        : getDefaultModel(selectedProvider);
      const workspaces = new Map<string, Workspace>();
      (savedWorkspaces ?? []).forEach((workspace) => {
        const normalized = workspaceFromPath(workspace.path, workspace.lastOpenedAt);
        if (normalized) workspaces.set(normalized.id, { ...normalized, name: workspace.name || normalized.name });
      });
      if (workspaces.size === 0) {
        const legacyWorkspace = getLegacyWorkspace();
        if (legacyWorkspace) workspaces.set(legacyWorkspace.id, legacyWorkspace);
      }
      (data ?? []).forEach((session) => {
        const workspace = workspaceFromPath(session.workingDir ?? "", session.updatedAt ?? Date.now());
        if (workspace && !workspaces.has(workspace.id)) {
          workspaces.set(workspace.id, workspace);
        }
      });
      const sortedWorkspaceIds = sortWorkspaces(workspaces.values()).map((workspace) => workspace.id);
      const activeWorkspaceId = savedActiveWorkspaceId && workspaces.has(savedActiveWorkspaceId)
        ? savedActiveWorkspaceId
        : sortedWorkspaceIds[0] ?? null;
      if (data && data.length > 0) {
        const sessions = new Map<string, SessionState>();
        const workflowBySession = new Map<string, WorkflowState>();
        const deliverySummaryBySession = new Map<string, DeliverySummary>();
        const hydratedAt = Date.now();
        for (const s of data) {
          const blocks = await loadBlocks(s.id);
          const workingDir = normalizeWorkspacePath(s.workingDir ?? "");
          const workspaceId = s.workspaceId && workspaces.has(s.workspaceId)
            ? s.workspaceId
            : workingDir || activeWorkspaceId;
          // Backend sessions don't survive restarts — force stopped
          sessions.set(s.id, {
            ...s,
            workingDir: workspaceId,
            workspaceId,
            createdAt: s.createdAt ?? hydratedAt,
            updatedAt: s.updatedAt ?? s.createdAt ?? hydratedAt,
            blocks,
            costUsd: 0,
            streaming: false,
            status: "stopped" as const,
          });
          if (s.workflowState) {
            workflowBySession.set(s.id, s.workflowState);
          }
          const latestDeliverySummary = s.deliverySummary ?? latestDeliverySummaryFromBlocks(blocks);
          if (latestDeliverySummary) {
            deliverySummaryBySession.set(s.id, latestDeliverySummary);
          }
        }
        const workspaceScopedSessionIds = workspaceSessionIds(sessions, activeWorkspaceId);
        const fallbackActiveSessionId = workspaceScopedSessionIds[workspaceScopedSessionIds.length - 1] ?? null;
        const activeSessionId = savedActiveSessionId && workspaceScopedSessionIds.includes(savedActiveSessionId)
          ? savedActiveSessionId
          : fallbackActiveSessionId;
        set({
          sessions,
          activeSessionId,
          workspaces,
          activeWorkspaceId,
          workflowBySession,
          deliverySummaryBySession,
          hydrated: true,
          theme: (savedTheme as "light" | "dark") || get().theme,
          selectedProvider,
          selectedModel,
        });
        if (activeSessionId) {
          if (tauriRuntime) {
            persistBackendAppMetadata({
              workspaces,
              activeWorkspaceId,
              activeSessionId,
              selectedProvider,
              selectedModel,
            });
          } else {
            idbSet(ACTIVE_SESSION_KEY, activeSessionId).catch(() => {});
          }
        } else if (tauriRuntime) {
          persistBackendAppMetadata({
            workspaces,
            activeWorkspaceId,
            activeSessionId: null,
            selectedProvider,
            selectedModel,
          });
        } else {
          idbDel(ACTIVE_SESSION_KEY).catch(() => {});
        }
        persistSessions(sessions, workflowBySession, deliverySummaryBySession);
        persistWorkspaces(workspaces, activeWorkspaceId);
      } else {
        set({
          workspaces,
          activeWorkspaceId,
          hydrated: true,
          theme: (savedTheme as "light" | "dark") || get().theme,
          selectedProvider,
          selectedModel,
        });
        if (tauriRuntime) {
          persistBackendAppMetadata({
            workspaces,
            activeWorkspaceId,
            activeSessionId: null,
            selectedProvider,
            selectedModel,
          });
        } else {
          idbDel(ACTIVE_SESSION_KEY).catch(() => {});
        }
        persistWorkspaces(workspaces, activeWorkspaceId);
      }
    } catch {
      set({ hydrated: true });
    }
  },
  theme: (typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches)
    ? "dark"
    : "light",

  setActiveSession: (id) => {
    set({ activeSessionId: id });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces: get().workspaces,
        activeWorkspaceId: get().activeWorkspaceId,
        activeSessionId: id,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else if (id) {
      idbSet(ACTIVE_SESSION_KEY, id).catch(() => {});
    } else {
      idbDel(ACTIVE_SESSION_KEY).catch(() => {});
    }
  },

  setActiveWorkspace: (id) => {
    const workspaces = new Map(get().workspaces);
    const activeWorkspaceId = id && workspaces.has(id) ? id : null;
    const scopedSessionIds = workspaceSessionIds(get().sessions, activeWorkspaceId);
    const currentSessionId = get().activeSessionId;
    const activeSessionId = currentSessionId && scopedSessionIds.includes(currentSessionId)
      ? currentSessionId
      : scopedSessionIds[scopedSessionIds.length - 1] ?? null;
    set({ activeWorkspaceId, activeSessionId });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces,
        activeWorkspaceId,
        activeSessionId,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else {
      persistWorkspaces(workspaces, activeWorkspaceId);
    }
    if (!hasTauriRuntime() && activeSessionId) {
      idbSet(ACTIVE_SESSION_KEY, activeSessionId).catch(() => {});
    } else if (!hasTauriRuntime()) {
      idbDel(ACTIVE_SESSION_KEY).catch(() => {});
    }
  },

  upsertWorkspace: (workspace) => {
    const normalized = workspaceFromPath(workspace.path, Date.now());
    if (!normalized) return;
    const workspaces = new Map(get().workspaces);
    const nextWorkspace = {
      ...normalized,
      name: workspace.name || normalized.name,
      lastOpenedAt: workspace.lastOpenedAt || normalized.lastOpenedAt,
    };
    workspaces.set(nextWorkspace.id, nextWorkspace);
    const scopedSessionIds = workspaceSessionIds(get().sessions, nextWorkspace.id);
    const activeSessionId = scopedSessionIds[scopedSessionIds.length - 1] ?? null;
    set({ workspaces, activeWorkspaceId: nextWorkspace.id, activeSessionId });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces,
        activeWorkspaceId: nextWorkspace.id,
        activeSessionId,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else {
      persistWorkspaces(workspaces, nextWorkspace.id);
    }
    if (!hasTauriRuntime() && activeSessionId) {
      idbSet(ACTIVE_SESSION_KEY, activeSessionId).catch(() => {});
    } else if (!hasTauriRuntime()) {
      idbDel(ACTIVE_SESSION_KEY).catch(() => {});
    }
  },

  removeWorkspace: (id) => {
    const workspaces = new Map(get().workspaces);
    workspaces.delete(id);
    const nextWorkspaceId = sortWorkspaces(workspaces.values())[0]?.id ?? null;
    const scopedSessionIds = workspaceSessionIds(get().sessions, nextWorkspaceId);
    const activeSessionId = scopedSessionIds[scopedSessionIds.length - 1] ?? null;
    set({ workspaces, activeWorkspaceId: nextWorkspaceId, activeSessionId });
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces,
        activeWorkspaceId: nextWorkspaceId,
        activeSessionId,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else {
      persistWorkspaces(workspaces, nextWorkspaceId);
    }
    if (!hasTauriRuntime() && activeSessionId) {
      idbSet(ACTIVE_SESSION_KEY, activeSessionId).catch(() => {});
    } else if (!hasTauriRuntime()) {
      idbDel(ACTIVE_SESSION_KEY).catch(() => {});
    }
  },

  setMemories: (memories) => set({ memories }),

  upsertMemory: (memory) => {
    const memories = get().memories.filter((existing) => existing.id !== memory.id);
    if (memory.status === "forgotten" || memory.status === "archived") {
      const selectedContextBySession = new Map(get().selectedContextBySession);
      selectedContextBySession.forEach((selected, sessionId) => {
        const nextSelected = selected.filter((item) => item.memory_id !== memory.id);
        if (nextSelected.length === 0) {
          selectedContextBySession.delete(sessionId);
        } else if (nextSelected.length !== selected.length) {
          selectedContextBySession.set(sessionId, nextSelected);
        }
      });
      set({ memories, selectedContextBySession });
      return;
    }
    set({ memories: [memory, ...memories] });
  },

  setForgeWikiContext: (sessionId, selected) => {
    const forgeWikiContextBySession = new Map(get().forgeWikiContextBySession);
    forgeWikiContextBySession.set(sessionId, selected);
    set({ forgeWikiContextBySession });
  },

  toggleMcpContext: (sessionId, selection) => {
    const mcpContextBySession = new Map(get().mcpContextBySession);
    const current = mcpContextBySession.get(sessionId) ?? [];
    const exists = current.some((item) => sameMcpContextSelection(item, selection));
    const next = exists
      ? current.filter((item) => !sameMcpContextSelection(item, selection))
      : [...current, selection];
    if (next.length === 0) {
      mcpContextBySession.delete(sessionId);
    } else {
      mcpContextBySession.set(sessionId, next);
    }
    set({ mcpContextBySession });
  },

  clearMcpContext: (sessionId) => {
    const mcpContextBySession = new Map(get().mcpContextBySession);
    mcpContextBySession.delete(sessionId);
    set({ mcpContextBySession });
  },

  upsertForgeWikiProposal: (sessionId, proposal) => {
    const forgeWikiProposalsBySession = new Map(get().forgeWikiProposalsBySession);
    const proposals = forgeWikiProposalsBySession.get(sessionId) ?? [];
    const nextProposals = [
      proposal,
      ...proposals.filter((existing) => existing.id !== proposal.id),
    ];
    forgeWikiProposalsBySession.set(sessionId, nextProposals);
    set({ forgeWikiProposalsBySession });
  },

  addSession: (id, provider, model, workingDir) => {
    const sessions = new Map(get().sessions);
    const existing = sessions.get(id);
    const normalizedWorkspace = workspaceFromPath(workingDir || get().activeWorkspaceId || "");
    const workspaces = new Map(get().workspaces);
    if (normalizedWorkspace) {
      workspaces.set(normalizedWorkspace.id, normalizedWorkspace);
    }
    const workspaceId = normalizedWorkspace?.id ?? get().activeWorkspaceId;
    sessions.set(id, {
      id,
      agentType: provider,
      model,
      workingDir: workspaceId,
      workspaceId,
      createdAt: existing?.createdAt ?? Date.now(),
      updatedAt: Date.now(),
      contextWindowTokens: existing?.contextWindowTokens ?? getModelContextWindow(model),
      status: "running",
      blocks: existing?.blocks ?? [],
      costUsd: existing?.costUsd ?? 0,
      streaming: existing?.streaming ?? false,
    });
    set({ sessions, workspaces, activeWorkspaceId: workspaceId, activeSessionId: id });
    persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces,
        activeWorkspaceId: workspaceId,
        activeSessionId: id,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else {
      persistWorkspaces(workspaces, workspaceId);
      idbSet(ACTIVE_SESSION_KEY, id).catch(() => {});
    }
  },

  removeSession: (id) => {
    const sessions = new Map(get().sessions);
    const selectedContextBySession = new Map(get().selectedContextBySession);
    const forgeWikiContextBySession = new Map(get().forgeWikiContextBySession);
    const mcpContextBySession = new Map(get().mcpContextBySession);
    const mcpContextStatusBySession = new Map(get().mcpContextStatusBySession);
    const forgeWikiProposalsBySession = new Map(get().forgeWikiProposalsBySession);
    const workflowBySession = new Map(get().workflowBySession);
    const agentTurnBySession = new Map(get().agentTurnBySession);
    const firstLoopDraftBySession = new Map(get().firstLoopDraftBySession);
    const deliverySummaryBySession = new Map(get().deliverySummaryBySession);
    sessions.delete(id);
    selectedContextBySession.delete(id);
    forgeWikiContextBySession.delete(id);
    mcpContextBySession.delete(id);
    mcpContextStatusBySession.delete(id);
    forgeWikiProposalsBySession.delete(id);
    workflowBySession.delete(id);
    agentTurnBySession.delete(id);
    firstLoopDraftBySession.delete(id);
    deliverySummaryBySession.delete(id);
    const remainingSessionIds = workspaceSessionIds(sessions, get().activeWorkspaceId);
    const activeSessionId =
      get().activeSessionId === id
        ? remainingSessionIds[remainingSessionIds.length - 1] ?? null
        : get().activeSessionId;
    set({
      sessions,
      activeSessionId,
      selectedContextBySession,
      forgeWikiContextBySession,
      mcpContextBySession,
      mcpContextStatusBySession,
      forgeWikiProposalsBySession,
      workflowBySession,
      agentTurnBySession,
      firstLoopDraftBySession,
      deliverySummaryBySession,
    });
    clearPendingBlockPersist(id);
    // Await both to prevent races with async persist from other actions
    if (hasTauriRuntime()) {
      persistBackendAppMetadata({
        workspaces: get().workspaces,
        activeWorkspaceId: get().activeWorkspaceId,
        activeSessionId,
        selectedProvider: get().selectedProvider,
        selectedModel: get().selectedModel,
      });
    } else {
      Promise.all([
        persistSessions(sessions, workflowBySession, deliverySummaryBySession),
        idbDel(BLOCKS_PREFIX + id).catch(() => {}),
        activeSessionId
          ? idbSet(ACTIVE_SESSION_KEY, activeSessionId).catch(() => {})
          : idbDel(ACTIVE_SESSION_KEY).catch(() => {}),
      ]).catch(() => {});
    }
  },

  setWorkflowState: (sessionId, workflow) => {
    const workflowBySession = new Map(get().workflowBySession);
    workflowBySession.set(sessionId, workflow);
    set({ workflowBySession });
    persistSessions(get().sessions, workflowBySession, get().deliverySummaryBySession);
  },

  setFirstLoopDraft: (sessionId, draft) => {
    const firstLoopDraftBySession = new Map(get().firstLoopDraftBySession);
    if (draft) {
      firstLoopDraftBySession.set(sessionId, draft);
    } else {
      firstLoopDraftBySession.delete(sessionId);
    }
    set({ firstLoopDraftBySession });
  },

  updateBlock: (sessionId: string, blockId: string, patch: Partial<BlockState>) => {
    const sessions = new Map(get().sessions);
    const session = sessions.get(sessionId);
    if (!session) return;
    const blocks = session.blocks.map((b) =>
      b.block_id === blockId ? { ...b, ...patch } : b
    );
    sessions.set(sessionId, { ...session, blocks });
    set({ sessions });
    persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
    persistBlocks(sessionId, blocks);
  },

  updateSessionStatus: (id, status) => {
    const sessions = new Map(get().sessions);
    const session = sessions.get(id);
    if (session) {
      sessions.set(id, { ...session, status });
    }
    set({ sessions });
    persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
  },

  addUserMessage: (sessionId, text) => {
    const sessions = new Map(get().sessions);
    const session = sessions.get(sessionId);
    if (!session) return;
    const blocks = [...session.blocks];
    // Remove any stale pending blocks
    const filtered = blocks.filter(b => b.event_type !== "pending");
    const blockId = crypto.randomUUID();
    // Add user message
    filtered.push({
      block_id: blockId,
      event_type: "user_message",
      content: text,
      isComplete: true,
      metadata: {},
    });
    // Add pending indicator — removed when first real event arrives
    filtered.push({
      block_id: "pending-" + crypto.randomUUID(),
      event_type: "pending",
      content: "",
      isComplete: false,
      metadata: {},
    });
    sessions.set(sessionId, { ...session, blocks: filtered, updatedAt: Date.now() });
    set({ sessions });
    persistBlocks(sessionId, filtered);
  },

  dispatchOutputEvent: (event) => {
    const { session_id, event_type } = event;

    if (event_type === "workflow_updated") {
      get().setWorkflowState(session_id, event.state);
      return;
    }

    if (event_type === "agent_turn_updated") {
      const agentTurnBySession = new Map(get().agentTurnBySession);
      agentTurnBySession.set(session_id, event.state);
      set({ agentTurnBySession });
      return;
    }

    if (event_type === "delivery_summary") {
      const sessionBlocks = get().sessions.get(session_id)?.blocks ?? [];
      const shouldDedupeReplay = isSameAsLastDeliveryBlock(sessionBlocks, event.summary);
      const deliverySummaryBySession = new Map(get().deliverySummaryBySession);
      deliverySummaryBySession.set(session_id, event.summary);
      set({ deliverySummaryBySession });
      persistSessions(get().sessions, get().workflowBySession, deliverySummaryBySession);
      if (shouldDedupeReplay) {
        return;
      }
    }

    if (event_type === "memory_selection") {
      const selectedContextBySession = new Map(get().selectedContextBySession);
      selectedContextBySession.set(session_id, event.selected);
      set({ selectedContextBySession });
      return;
    }

    if (event_type === "memory_candidate" || event_type === "memory_updated") {
      get().upsertMemory(event.memory);
      return;
    }

    if (event_type === "forge_wiki_context_selected") {
      get().setForgeWikiContext(session_id, event.selected);
      return;
    }

    if (event_type === "mcp_context_status") {
      const mcpContextStatusBySession = new Map(get().mcpContextStatusBySession);
      const current = new Map(mcpContextStatusBySession.get(session_id) ?? []);
      current.set(event.source_id, {
        source_id: event.source_id,
        status: event.status,
        message: event.message ?? null,
      });
      mcpContextStatusBySession.set(session_id, current);
      set({ mcpContextStatusBySession });
      return;
    }

    if (event_type === "forge_wiki_update_proposed" || event_type === "forge_wiki_updated") {
      get().upsertForgeWikiProposal(session_id, event.proposal);
      return;
    }

    const sessions = new Map(get().sessions);
    let session = sessions.get(session_id);

    if (!session) {
      // If session_started arrives before addSession, create it from the event
      if (event_type === "session_started") {
        const se = event as Extract<StreamEvent, { event_type: "session_started" }>;
        session = {
          id: session_id,
          agentType: se.agent_type,
          model: se.model,
          workingDir: get().activeWorkspaceId,
          workspaceId: get().activeWorkspaceId,
          contextWindowTokens: se.context_window_tokens ?? getModelContextWindow(se.model),
          status: "running",
          blocks: [],
          costUsd: 0,
          streaming: false,
        };
        sessions.set(session_id, session);
        set({ sessions });
        persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
        return;
      }
      return;
    }

    let blocks = [...session.blocks];

    // Remove pending indicator when first real event arrives
    if ((event_type as string) !== "pending" && event_type !== "session_started"
        && event_type !== "session_status" && event_type !== "session_stopped") {
      blocks = blocks.filter(b => b.event_type !== "pending");
    }

    // Handle block accumulation for streaming events
    const chunkTypes = [
      "thinking_chunk",
      "text_chunk",
      "shell_output",
    ];

    const endTypes = [
      "thinking_end",
      "text_end",
      "shell_end",
      "tool_call_end",
    ];

    // Session lifecycle events
    if (event_type === "session_started") {
      // Update session info from the backend event
      const startedEvent = event as Extract<StreamEvent, { event_type: "session_started" }>;
      sessions.set(session_id, {
        ...session,
        agentType: startedEvent.agent_type,
        model: startedEvent.model,
        workingDir: session.workingDir ?? get().activeWorkspaceId,
        workspaceId: session.workspaceId ?? get().activeWorkspaceId,
        contextWindowTokens: startedEvent.context_window_tokens ?? getModelContextWindow(startedEvent.model),
        status: "running",
        streaming: false,
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      return;
    }

    if (event_type === "session_stopped") {
      sessions.set(session_id, {
        ...session,
        status: "stopped",
        blocks,
        streaming: false,
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      persistBlocksNow(session_id, blocks);
      return;
    }

    if (event_type === "usage") {
      const ue = event as Extract<StreamEvent, { event_type: "usage" }>;
      sessions.set(session_id, {
        ...session,
        costUsd: (session.costUsd || 0) + ue.estimated_cost_usd,
        blocks,
      });
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      return;
    }

    if (event_type === "session_status") {
      const statusEvent = event as Extract<StreamEvent, { event_type: "session_status" }>;
      const status = statusEvent.status === "error" ? "error" : "running";
      sessions.set(session_id, {
        ...session,
        status,
        blocks,
        streaming: statusEvent.status === "working",
      });
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    if (event_type === "error") {
      const errorEvent = event as Extract<StreamEvent, { event_type: "error" }>;
      if (
        errorEvent.code === "missing_api_key" &&
        blocks.some((block) => block.event_type === "error" && block.metadata?.code === "missing_api_key")
      ) {
        return;
      }
      const newBlocks = [
        ...blocks,
        {
          block_id: errorEvent.block_id,
          event_type: "error",
          content: errorEvent.message,
          metadata: { code: errorEvent.code },
          isComplete: true,
        },
      ];
      sessions.set(session_id, {
        ...session,
        blocks: newBlocks,
      });
      set({ sessions });
      persistBlocksNow(session_id, newBlocks);
      return;
    }

    // For tool_call_result, find the tool_call block and merge
    if (event_type === "tool_call_result") {
      const resultEvent = event as Extract<StreamEvent, { event_type: "tool_call_result" }>;
      // Try exact block_id match first, then fall back to last empty tool/shell/thinking/read block
      let existingIdx = blocks.findIndex((b) =>
        (b.event_type === "tool_call" || b.event_type === "shell" || b.event_type === "thinking") && b.block_id === resultEvent.block_id
      );
      if (existingIdx < 0) {
        // Block IDs from streaming vs execution don't match — find the most recent block
        // of any tool-related type that hasn't received its result yet
        existingIdx = [...blocks].reverse().findIndex((b) =>
          (b.event_type === "tool_call" || b.event_type === "shell" || b.event_type === "thinking")
          && (!b.content || b.content === "")
        );
        if (existingIdx >= 0) {
          existingIdx = blocks.length - 1 - existingIdx;
        }
      }
      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          content: resultEvent.result,
          isComplete: true,
          metadata: {
            ...blocks[existingIdx].metadata,
            is_error: resultEvent.is_error,
            duration_ms: resultEvent.duration_ms,
          },
        };
      } else {
        // Fallback: create standalone block with content
        blocks.push({
          block_id: resultEvent.block_id,
          event_type: "tool_call",
          content: resultEvent.result,
          isComplete: true,
          metadata: {
            is_error: resultEvent.is_error,
            duration_ms: resultEvent.duration_ms,
            tool_name: "Tool",
          },
        });
      }
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    // For chunk events, find existing block and append content
    if (chunkTypes.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = blocks.findIndex((b) => b.block_id === blockIdEvent.block_id);
      const content = "content" in event ? (event as { content: string }).content : "";

      if (existingIdx >= 0) {
        blocks[existingIdx] = {
          ...blocks[existingIdx],
          content: blocks[existingIdx].content + content,
        };
      } else {
        // No existing block — create one (handles PTY output that emits chunks without a start event)
        const blockType = event_type === "thinking_chunk" ? "thinking"
          : event_type === "shell_output" ? "shell"
          : "text";
        blocks.push({
          block_id: blockIdEvent.block_id,
          event_type: blockType,
          content,
          isComplete: false,
          metadata: {},
        });
      }
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocks(session_id, blocks);
      return;
    }

    // For end events, mark block as complete (except tool_call_end — results set isComplete later)
    if (endTypes.includes(event_type)) {
      const blockIdEvent = event as { block_id: string };
      const existingIdx = blocks.findIndex((b) => b.block_id === blockIdEvent.block_id);
      if (existingIdx >= 0) {
        if (event_type !== "tool_call_end") {
          blocks[existingIdx] = { ...blocks[existingIdx], isComplete: true };
        }
        // Capture exit_code for shell blocks
        if (event_type === "shell_end") {
          const se = event as Extract<StreamEvent, { event_type: "shell_end" }>;
          blocks[existingIdx] = {
            ...blocks[existingIdx],
            metadata: { ...blocks[existingIdx].metadata, exit_code: se.exit_code },
          };
        }
      }
      sessions.set(session_id, { ...session, blocks });
      set({ sessions });
      persistBlocksNow(session_id, blocks);
      return;
    }

    // For all other events, create a new block
    const newBlock = eventToBlock(event);
    if (newBlock) {
      blocks.push(newBlock);
    }

    sessions.set(session_id, { ...session, blocks });
    set({ sessions });
    persistBlocks(session_id, blocks);
  },

  setPendingInput: (text) => set({ pendingInput: text }),

  setTheme: (theme) => {
    set({ theme });
    idbSet("tui-theme", theme).catch(() => {});
  },
}));

function eventToBlock(event: StreamEvent): BlockState | null {
  const base = {
    block_id: "block_id" in event ? (event as { block_id: string }).block_id : "",
    isComplete: false,
    metadata: {} as Record<string, unknown>,
  };

  switch (event.event_type) {
    case "user_message":
      return {
        ...base,
        event_type: "user_message",
        content: event.content,
        isComplete: true,
      };
    case "thinking_start":
      return { ...base, event_type: "thinking", content: "", metadata: {} };
    case "text_start":
      return { ...base, event_type: "text", content: "" };
    case "tool_call_start":
      return {
        ...base,
        event_type: "tool_call",
        content: "",
        metadata: {
          tool_name: event.tool_name,
          tool_input: event.tool_input,
        },
      };
    case "tool_call_result":
      return {
        ...base,
        event_type: "tool_call_result",
        content: event.result,
        metadata: {
          is_error: event.is_error,
          duration_ms: event.duration_ms,
        },
      };
    case "diff_view":
      return {
        ...base,
        event_type: "diff_view",
        content: event.new_content,
        metadata: {
          file_path: event.file_path,
          old_content: event.old_content,
        },
      };
    case "shell_start":
      return {
        ...base,
        event_type: "shell",
        content: "",
        metadata: { command: event.command },
      };
    case "confirm_ask":
      return {
        ...base,
        event_type: "confirm_ask",
        content: event.question,
        metadata: {
          kind: event.kind,
          boundary: event.boundary ?? null,
        },
      };
    case "context_compacted":
      return {
        ...base,
        event_type: "context_compacted",
        content: event.summary,
        metadata: {
          retained_messages: event.retained_messages,
          compacted_messages: event.compacted_messages,
          estimated_tokens_before: event.estimated_tokens_before,
          estimated_tokens_after: event.estimated_tokens_after,
        },
        isComplete: true,
      };
    case "delivery_summary":
      return {
        ...base,
        event_type: "delivery_summary",
        content: "本轮交付",
        metadata: {
          summary: event.summary,
        },
        isComplete: true,
      };
    default:
      return null;
  }
}

function deliverySummariesEqual(left: DeliverySummary | null, right: DeliverySummary | null) {
  if (!left || !right) return false;
  return (
    (left.project_path ?? null) === (right.project_path ?? null) &&
    left.preview_label === right.preview_label &&
    left.checkpoint_label === right.checkpoint_label &&
    left.next_action === right.next_action &&
    (left.verification_label ?? null) === (right.verification_label ?? null) &&
    (left.verification_status ?? null) === (right.verification_status ?? null) &&
    (left.verification_command ?? null) === (right.verification_command ?? null) &&
    (left.record_label ?? null) === (right.record_label ?? null) &&
    (left.record_status ?? null) === (right.record_status ?? null) &&
    JSON.stringify(left.record_target_pages ?? []) === JSON.stringify(right.record_target_pages ?? [])
  );
}

// Selector hooks
export const useActiveSession = () =>
  useStore((s) => {
    if (!s.activeSessionId) return null;
    return s.sessions.get(s.activeSessionId) ?? null;
  });

export const useSessionList = () =>
  useStore((s) => {
    if (!s.activeWorkspaceId) {
      return sortSessionsByRecency(Array.from(s.sessions.values()).filter((session) => !session.workspaceId && !session.workingDir));
    }
    return sortSessionsByRecency(Array.from(s.sessions.values()).filter((session) =>
      session.workspaceId === s.activeWorkspaceId || session.workingDir === s.activeWorkspaceId
    ));
  });

function sortSessionsByRecency(sessions: SessionState[]) {
  return [...sessions].sort((a, b) => sessionTime(b) - sessionTime(a));
}

function sessionTime(session: SessionState) {
  return session.updatedAt ?? session.createdAt ?? 0;
}

export const useWorkspaceList = () =>
  useStore((s) => sortWorkspaces(s.workspaces.values()));

export const useActiveWorkspace = () =>
  useStore((s) => s.activeWorkspaceId ? s.workspaces.get(s.activeWorkspaceId) ?? null : null);

export const useActiveBlocks = () =>
  useStore((s) => {
    if (!s.activeSessionId) return [];
    return s.sessions.get(s.activeSessionId)?.blocks ?? [];
  });
