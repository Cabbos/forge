import { get as idbGet, set as idbSet, del as idbDel } from "idb-keyval";
import type { DeliverySummary, SessionState, WorkflowState } from "../lib/protocol";
import { queryClient } from "../lib/query-client";
import { queryKeys } from "../hooks/queries/queryKeys";
import {
  hasTauriRuntime,
  listSessions,
  loadAppMetadata,
} from "../lib/tauri";
import type { Workspace } from "../lib/workspaces";
import {
  normalizeWorkspacePath,
  sortWorkspaces,
  workspaceFromPath,
} from "../lib/workspaces";
import {
  getDefaultModel,
  modelBelongsToProvider,
  normalizeProviderId,
} from "../lib/providers";
import {
  ACTIVE_SESSION_KEY,
  ACTIVE_WORKSPACE_KEY,
  MODEL_KEY,
  PERSIST_KEY,
  PROVIDER_KEY,
  WORKSPACES_KEY,
  loadBlocks,
  persistBackendAppMetadata,
  persistSessions,
  persistWorkspaces,
} from "./persistence";
import {
  closeInterruptedConfirmBlocks,
  latestDeliverySummaryFromBlocks,
  transcriptEventsToBlocks,
} from "./blocks";
import {
  getLegacyWorkspace,
  persistedSessionFromBackend,
  workspaceSessionIds,
} from "./session-utils";
import type { AppStore, PersistedSession } from "./types";
import { usageProjectionFromProviderUsageBlocks } from "./usage-ledger";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;

export function createHydrateAction(set: StoreSet, get: StoreGet) {
  return async () => {
    try {
      const tauriRuntime = hasTauriRuntime();
      const backendMetadata = tauriRuntime
        ? await queryClient.fetchQuery({ queryKey: queryKeys.appMetadata, queryFn: () => loadAppMetadata() }).catch(() => null)
        : null;
      const backendSessions = tauriRuntime
        ? await queryClient.fetchQuery({ queryKey: queryKeys.sessions, queryFn: () => listSessions() }).catch(() => [])
        : [];
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
          const loadedBlocks = await loadBlocks(s.id, transcriptEventsToBlocks);
          const blocks = closeInterruptedConfirmBlocks(loadedBlocks, "session_restored");
          const restoredUsage = usageProjectionFromProviderUsageBlocks(
            blocks,
            s.contextWindowTokens,
            s.contextUsage,
            hydratedAt,
          );
          const workingDir = normalizeWorkspacePath(s.workingDir ?? "");
          const workspaceId = s.workspaceId && workspaces.has(s.workspaceId)
            ? s.workspaceId
            : workingDir || activeWorkspaceId;
          sessions.set(s.id, {
            ...s,
            workingDir: workspaceId,
            workspaceId,
            createdAt: s.createdAt ?? hydratedAt,
            updatedAt: s.updatedAt ?? s.createdAt ?? hydratedAt,
            blocks,
            costUsd: sanitizePersistedCost(s.costUsd) ?? restoredUsage?.costUsd ?? 0,
            contextUsage: restoredUsage?.replayedCompactedContext
              ? restoredUsage.contextUsage
              : s.contextUsage ?? restoredUsage?.contextUsage ?? null,
            usageLedger: s.usageLedger ?? restoredUsage?.usageLedger ?? null,
            streaming: false,
            // For Tauri, respect the backend-reported status (including "resuming").
            // For IndexedDB-only hydration, force "stopped" so stale persisted state
            // doesn't show a phantom running/resuming session.
            status: tauriRuntime ? s.status : ("stopped" as const),
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
  };
}

function sanitizePersistedCost(value: number | null | undefined) {
  return typeof value === "number" && Number.isFinite(value) ? Math.max(0, value) : null;
}
