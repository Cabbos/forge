export interface Workspace {
  id: string;
  name: string;
  path: string;
  lastOpenedAt: number;
}

export function normalizeWorkspacePath(path?: string | null): string {
  const normalized = (path ?? "").trim().replace(/\/+$/, "");
  if (!normalized || isBroadWorkspacePath(normalized)) return "";
  return normalized;
}

export function isBroadWorkspacePath(path?: string | null): boolean {
  const normalized = (path ?? "").trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return true;
  return (
    normalized === "/Users" ||
    normalized === "/home" ||
    /^\/Users\/[^/]+$/.test(normalized) ||
    /^\/home\/[^/]+$/.test(normalized)
  );
}

export function workspaceFromPath(path: string, lastOpenedAt = Date.now()): Workspace | null {
  const normalized = normalizeWorkspacePath(path);
  if (!normalized) return null;
  return {
    id: normalized,
    name: workspaceNameFromPath(normalized),
    path: normalized,
    lastOpenedAt,
  };
}

export function workspaceNameFromPath(path: string): string {
  return path.split("/").filter(Boolean).pop() || path;
}

export function sortWorkspaces(workspaces: Iterable<Workspace>): Workspace[] {
  return Array.from(workspaces).sort((a, b) => b.lastOpenedAt - a.lastOpenedAt);
}
