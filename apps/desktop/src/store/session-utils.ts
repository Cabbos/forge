import type {
  ContextUsageState,
  SessionState,
} from "../lib/protocol";
import type { McpContextSelection, SessionInfo } from "../lib/tauri";
import { workspaceFromPath, type Workspace } from "../lib/workspaces";
import { LEGACY_WORKING_DIR_KEY } from "./persistence";
import type { PersistedSession } from "./types";

export function persistedSessionFromBackend(info: SessionInfo): PersistedSession {
  return {
    id: info.id,
    agentType: info.provider,
    model: info.model,
    workingDir: info.working_dir ?? null,
    workspaceId: info.working_dir ?? null,
    createdAt: info.created_at_ms ?? null,
    updatedAt: info.updated_at_ms ?? info.created_at_ms ?? null,
    contextWindowTokens: info.context_window_tokens ?? null,
    contextUsage: null,
    usageLedger: null,
    status: coerceSessionStatus(info.status),
    workflowState: info.latest_workflow ?? null,
    deliverySummary: info.latest_delivery ?? null,
  };
}

function coerceSessionStatus(status: string): SessionState["status"] {
  if (status === "running" || status === "error" || status === "resuming") return status;
  return "stopped";
}

export function buildContextUsage(
  usedTokens: number | null,
  contextWindowTokens: number | null | undefined,
  source: ContextUsageState["source"],
  previous?: ContextUsageState | null,
  compacted?: { from: number | null; to: number | null },
): ContextUsageState {
  const safeUsed = typeof usedTokens === "number" && Number.isFinite(usedTokens)
    ? Math.max(0, Math.round(usedTokens))
    : null;
  const safeWindow = typeof contextWindowTokens === "number" && Number.isFinite(contextWindowTokens)
    ? Math.max(0, Math.round(contextWindowTokens))
    : null;
  const percentUsed = safeUsed !== null && safeWindow && safeWindow > 0
    ? Math.min(100, Math.round((safeUsed / safeWindow) * 100))
    : null;

  return {
    usedTokens: safeUsed,
    contextWindowTokens: safeWindow,
    percentUsed,
    source,
    lastUpdatedAt: Date.now(),
    lastCompactedAt: compacted ? Date.now() : previous?.lastCompactedAt ?? null,
    compactedFromTokens: compacted ? compacted.from : previous?.compactedFromTokens ?? null,
    compactedToTokens: compacted ? compacted.to : previous?.compactedToTokens ?? null,
  };
}

export function getLegacyWorkspace(): Workspace | null {
  if (typeof window === "undefined") return null;
  return workspaceFromPath(window.localStorage.getItem(LEGACY_WORKING_DIR_KEY) ?? "");
}

export function workspaceSessionIds(sessions: Map<string, SessionState>, workspaceId: string | null) {
  if (!workspaceId) {
    return Array.from(sessions.values())
      .filter((session) => !session.workspaceId && !session.workingDir)
      .map((session) => session.id);
  }
  return Array.from(sessions.values())
    .filter((session) => session.workspaceId === workspaceId || session.workingDir === workspaceId)
    .map((session) => session.id);
}

export function sameMcpContextSelection(a: McpContextSelection, b: McpContextSelection) {
  if (a.kind !== b.kind || a.server_id !== b.server_id) return false;
  return a.kind === "resource" && b.kind === "resource"
    ? a.uri === b.uri
    : a.kind === "prompt" && b.kind === "prompt" && a.name === b.name;
}

export function touchSession(session: SessionState, patch: Partial<SessionState> = {}): SessionState {
  return {
    ...session,
    ...patch,
    updatedAt: Date.now(),
  };
}
