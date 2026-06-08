import type { SessionState } from "../lib/protocol";
import { sortWorkspaces } from "../lib/workspaces";
import type { AppStore } from "./types";

type StoreHook = <T>(selector: (state: AppStore) => T) => T;

export function createStoreSelectors(useStore: StoreHook) {
  const useActiveSession = () =>
    useStore((s) => {
      if (!s.activeSessionId) return null;
      return s.sessions.get(s.activeSessionId) ?? null;
    });

  const useSessionList = () =>
    useStore((s) => {
      if (!s.activeWorkspaceId) {
        return sortSessionsByRecency(Array.from(s.sessions.values()).filter((session) => !session.workspaceId && !session.workingDir));
      }
      return sortSessionsByRecency(Array.from(s.sessions.values()).filter((session) =>
        session.workspaceId === s.activeWorkspaceId || session.workingDir === s.activeWorkspaceId
      ));
    });

  const useWorkspaceList = () =>
    useStore((s) => sortWorkspaces(s.workspaces.values()));

  const useActiveWorkspace = () =>
    useStore((s) => s.activeWorkspaceId ? s.workspaces.get(s.activeWorkspaceId) ?? null : null);

  const useActiveBlocks = () =>
    useStore((s) => {
      if (!s.activeSessionId) return [];
      return s.sessions.get(s.activeSessionId)?.blocks ?? [];
    });

  return {
    useActiveSession,
    useSessionList,
    useWorkspaceList,
    useActiveWorkspace,
    useActiveBlocks,
  };
}

function sortSessionsByRecency(sessions: SessionState[]) {
  return [...sessions].sort((a, b) => sessionTime(b) - sessionTime(a));
}

function sessionTime(session: SessionState) {
  return session.updatedAt ?? session.createdAt ?? 0;
}
