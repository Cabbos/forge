import type {
  ForgeWikiUpdateProposal,
  WikiMemory,
} from "@/lib/protocol";

export const EMPTY_FORGE_WIKI_PROPOSALS: ForgeWikiUpdateProposal[] = [];

export function normalizeProjectPath(path: string | null | undefined): string {
  return (path ?? "").trim().replace(/\/+$/, "");
}

export function memoryBelongsToCurrentContext(memory: WikiMemory, currentProjectPath: string): boolean {
  if (memory.scope === "user_profile" && !memory.project_path) return true;
  return currentProjectPath !== "" && normalizeProjectPath(memory.project_path) === currentProjectPath;
}

export function indexMemoriesById(memories: WikiMemory[] | null | undefined, currentProjectPath: string): Map<string, WikiMemory> {
  const byId = new Map<string, WikiMemory>();
  filterContextMemories(memories, currentProjectPath).forEach((memory) => {
    byId.set(memory.id, memory);
  });
  return byId;
}

export function filterCandidateMemories(memories: WikiMemory[] | null | undefined, currentProjectPath: string): WikiMemory[] {
  return filterContextMemories(memories, currentProjectPath).filter((memory) => memory.status === "candidate");
}

export function filterProjectMemories(memories: WikiMemory[] | null | undefined, currentProjectPath: string): WikiMemory[] {
  return (memories ?? []).filter(
    (memory) =>
      memory.scope === "project" &&
      currentProjectPath !== "" &&
      normalizeProjectPath(memory.project_path) === currentProjectPath &&
      (memory.status === "accepted" || memory.status === "pinned"),
  );
}

export function filterVisibleForgeWikiProposals(
  proposals: ForgeWikiUpdateProposal[] | null | undefined,
  currentProjectPath: string,
): ForgeWikiUpdateProposal[] {
  return (proposals ?? []).filter(
    (proposal) =>
      (proposal.status === "pending" ||
        proposal.status === "accepted" ||
        proposal.status === "discarded") &&
      (!currentProjectPath || normalizeProjectPath(proposal.project_path) === currentProjectPath),
  );
}

function filterContextMemories(memories: WikiMemory[] | null | undefined, currentProjectPath: string): WikiMemory[] {
  return (memories ?? []).filter((memory) => memoryBelongsToCurrentContext(memory, currentProjectPath));
}
