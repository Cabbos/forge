import { set as idbSet, del as idbDel } from "idb-keyval";
import { hasTauriRuntime } from "../lib/tauri";
import { sortWorkspaces, workspaceFromPath } from "../lib/workspaces";
import {
  ACTIVE_SESSION_KEY,
  persistBackendAppMetadata,
  persistWorkspaces,
} from "./persistence";
import { workspaceSessionIds } from "./session-utils";
import type { AppStore } from "./types";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;
type WorkspaceActions = Pick<
  AppStore,
  "setActiveSession" | "setActiveWorkspace" | "upsertWorkspace" | "removeWorkspace"
>;

export function createWorkspaceActions(set: StoreSet, get: StoreGet): WorkspaceActions {
  return {
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
  };
}
