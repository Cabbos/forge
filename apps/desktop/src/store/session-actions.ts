import { set as idbSet, del as idbDel } from "idb-keyval";
import type { BlockState } from "../lib/protocol";
import { getModelContextWindow } from "../lib/providers";
import { hasTauriRuntime } from "../lib/tauri";
import { workspaceFromPath } from "../lib/workspaces";
import {
  ACTIVE_SESSION_KEY,
  BLOCKS_PREFIX,
  clearPendingBlockPersist,
  persistBackendAppMetadata,
  persistBlocks,
  persistSessions,
  persistWorkspaces,
} from "./persistence";
import { touchSession, workspaceSessionIds } from "./session-utils";
import type { AppStore } from "./types";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;
type SessionActions = Pick<
  AppStore,
  | "addSession"
  | "removeSession"
  | "setWorkflowState"
  | "setFirstLoopDraft"
  | "updateBlock"
  | "updateSessionStatus"
  | "addUserMessage"
>;

export function createSessionActions(set: StoreSet, get: StoreGet): SessionActions {
  return {
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
        contextUsage: existing?.contextUsage ?? null,
        usageLedger: existing?.usageLedger ?? null,
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
      const agentA2ABySession = new Map(get().agentA2ABySession);
      const subagentRuntimeByTask = new Map(get().subagentRuntimeByTask);
      const loopRuntimeByTask = new Map(get().loopRuntimeByTask);
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
      agentA2ABySession.delete(id);
      for (const key of subagentRuntimeByTask.keys()) {
        if (key.startsWith(`${id}:`)) subagentRuntimeByTask.delete(key);
      }
      for (const key of loopRuntimeByTask.keys()) {
        if (key.startsWith(`${id}:`)) loopRuntimeByTask.delete(key);
      }
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
        agentA2ABySession,
        subagentRuntimeByTask,
        loopRuntimeByTask,
      });
      clearPendingBlockPersist(id);
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
      const blocks = session.blocks.map((block) =>
        block.block_id === blockId ? { ...block, ...patch } : block
      );
      sessions.set(sessionId, touchSession(session, { blocks }));
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
      persistBlocks(sessionId, blocks);
    },

    updateSessionStatus: (id, status) => {
      const sessions = new Map(get().sessions);
      const session = sessions.get(id);
      if (session) {
        sessions.set(id, touchSession(session, { status }));
      }
      set({ sessions });
      persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
    },

    addUserMessage: (sessionId, text) => {
      const sessions = new Map(get().sessions);
      const session = sessions.get(sessionId);
      if (!session) return;
      const blocks = [...session.blocks];
      const filtered = blocks.filter((block) => block.event_type !== "pending");
      const blockId = crypto.randomUUID();
      filtered.push({
        block_id: blockId,
        event_type: "user_message",
        content: text,
        isComplete: true,
        metadata: {},
      });
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
  };
}
