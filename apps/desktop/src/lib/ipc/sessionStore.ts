import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";
import type {
  PruneSessionStoreInput,
  RenameSessionSnapshotInput,
  SessionSnapshotPruneReport,
  SessionSnapshotStoreStats,
  SessionSnapshotSummary,
} from "./types.ts";

export async function getSessionStoreStats(): Promise<SessionSnapshotStoreStats> {
  if (!hasTauriRuntime()) {
    return {
      total_snapshots: 0,
      corrupted_snapshots: 0,
      total_bytes: 0,
      oldest_updated_at_ms: null,
      newest_updated_at_ms: null,
      by_provider: {},
      by_workspace: {},
    };
  }
  return invoke<SessionSnapshotStoreStats>("get_session_store_stats");
}

export async function searchSessionStore(
  query: string,
): Promise<SessionSnapshotSummary[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<SessionSnapshotSummary[]>("search_session_store", { query });
}

export async function exportSessionStore(): Promise<unknown> {
  if (!hasTauriRuntime()) {
    throw new Error("Session store export is not available outside Tauri runtime.");
  }
  return invoke<unknown>("export_session_store");
}

export async function renameSessionSnapshot(
  input: RenameSessionSnapshotInput,
): Promise<SessionSnapshotSummary | null> {
  if (!hasTauriRuntime()) {
    throw new Error("Session snapshot rename is not available outside Tauri runtime.");
  }
  return invoke<SessionSnapshotSummary | null>("rename_session_snapshot", {
    sessionId: input.sessionId,
    summary: input.summary,
  });
}

export async function pruneSessionStore(
  input: PruneSessionStoreInput,
): Promise<SessionSnapshotPruneReport> {
  if (!hasTauriRuntime()) {
    throw new Error("Session store prune is not available outside Tauri runtime.");
  }
  return invoke<SessionSnapshotPruneReport>("prune_session_store", {
    keepRecent: input.keepRecent,
    olderThanMs: input.olderThanMs ?? null,
  });
}
