import { sameMcpContextSelection } from "./session-utils";
import type { AppStore } from "./types";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;
type ContextActions = Pick<
  AppStore,
  | "setMemories"
  | "upsertMemory"
  | "setForgeWikiContext"
  | "toggleMcpContext"
  | "clearMcpContext"
  | "upsertForgeWikiProposal"
>;

export function createContextActions(set: StoreSet, get: StoreGet): ContextActions {
  return {
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
  };
}
