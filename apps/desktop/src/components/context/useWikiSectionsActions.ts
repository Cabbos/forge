import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  acceptForgeWikiUpdateProposal,
  discardForgeWikiUpdateProposal,
  forgetMemory,
  initForgeWiki,
  pinMemory,
  updateMemory,
} from "@/lib/tauri";
import type { ForgeWikiUpdateProposal, WikiMemory } from "@/lib/protocol";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { FORGE_WIKI_INIT_OPERATION_ID, type DraftState } from "./WikiSectionTypes";

interface UseWikiSectionsActionsOptions {
  currentProjectPath: string;
  sessionId: string | null;
  draft: DraftState | null;
  memoriesById: Map<string, WikiMemory>;
  refresh: () => Promise<void>;
  isCurrentRequest: (projectAtStart: string, sessionAtStart: string | null) => boolean;
  beginBusy: (id: string) => number;
  clearBusy: (token: number, id: string) => void;
  setBusyId: (value: React.SetStateAction<string | null>) => void;
  setDraft: (value: React.SetStateAction<DraftState | null>) => void;
  setError: (value: React.SetStateAction<string>) => void;
  upsertForgeWikiProposal: (sessionId: string, proposal: ForgeWikiUpdateProposal) => void;
}

export function useWikiSectionsActions({
  currentProjectPath,
  sessionId,
  draft,
  memoriesById,
  refresh,
  isCurrentRequest,
  beginBusy,
  clearBusy,
  setBusyId,
  setDraft,
  setError,
  upsertForgeWikiProposal,
}: UseWikiSectionsActionsOptions) {
  const queryClient = useQueryClient();
  const handleInitForgeWiki = useCallback(async () => {
    const projectAtStart = currentProjectPath;
    const sessionAtStart = sessionId;
    if (!projectAtStart) return;

    const operationId = FORGE_WIKI_INIT_OPERATION_ID;
    const busyToken = beginBusy(operationId);
    setError("");
    try {
      await initForgeWiki(projectAtStart, sessionAtStart);
      if (!isCurrentRequest(projectAtStart, sessionAtStart)) return;
      await queryClient.invalidateQueries({ queryKey: queryKeys.forgeWikiState(projectAtStart, sessionAtStart) });
      await refresh();
    } catch (err) {
      if (isCurrentRequest(projectAtStart, sessionAtStart)) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      clearBusy(busyToken, operationId);
    }
  }, [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, queryClient, refresh, sessionId, setError]);

  const handleAcceptForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      const projectAtStart = currentProjectPath;
      const sessionAtStart = sessionId;
      const busyToken = beginBusy(proposal.id);
      setError("");
      try {
        const nextProposal = await acceptForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionAtStart ?? undefined,
        );
        if (sessionAtStart) upsertForgeWikiProposal(sessionAtStart, nextProposal);
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          await queryClient.invalidateQueries({ queryKey: queryKeys.forgeWikiState(projectAtStart, sessionAtStart) });
          await refresh();
        }
      } catch (err) {
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        clearBusy(busyToken, proposal.id);
      }
    },
    [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, queryClient, refresh, sessionId, setError, upsertForgeWikiProposal],
  );

  const handleDiscardForgeWikiProposal = useCallback(
    async (proposal: ForgeWikiUpdateProposal) => {
      const projectAtStart = currentProjectPath;
      const sessionAtStart = sessionId;
      const busyToken = beginBusy(proposal.id);
      setError("");
      try {
        const nextProposal = await discardForgeWikiUpdateProposal(
          proposal.project_path,
          proposal.id,
          sessionAtStart ?? undefined,
        );
        if (sessionAtStart) upsertForgeWikiProposal(sessionAtStart, nextProposal);
      } catch (err) {
        if (isCurrentRequest(projectAtStart, sessionAtStart)) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        clearBusy(busyToken, proposal.id);
      }
    },
    [beginBusy, clearBusy, currentProjectPath, isCurrentRequest, sessionId, setError, upsertForgeWikiProposal],
  );

  const startEdit = useCallback((memory: WikiMemory) => {
    setDraft({ memoryId: memory.id, title: memory.title, body: memory.body });
  }, [setDraft]);

  const saveDraft = useCallback(async () => {
    if (!draft) return;
    const memory = memoriesById.get(draft.memoryId);
    if (!memory) return;

    setBusyId(memory.id);
    setError("");
    try {
      await updateMemory(
        memory.id,
        {
          title: draft.title.trim() || memory.title,
          body: draft.body.trim() || memory.body,
          status: memory.status === "candidate" ? "accepted" : memory.status,
        },
        sessionId ?? undefined,
      );
      setDraft(null);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyId(null);
    }
  }, [draft, memoriesById, refresh, sessionId, setBusyId, setDraft, setError]);

  const handlePin = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await pinMemory(memoryId, sessionId ?? undefined);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [refresh, sessionId, setBusyId, setError],
  );

  const handleAccept = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await updateMemory(memoryId, { status: "accepted" }, sessionId ?? undefined);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [refresh, sessionId, setBusyId, setError],
  );

  const handleForget = useCallback(
    async (memoryId: string) => {
      setBusyId(memoryId);
      setError("");
      try {
        await forgetMemory(memoryId, sessionId ?? undefined);
        if (draft?.memoryId === memoryId) setDraft(null);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusyId(null);
      }
    },
    [draft?.memoryId, refresh, sessionId, setBusyId, setDraft, setError],
  );

  return {
    handleAccept,
    handleAcceptForgeWikiProposal,
    handleDiscardForgeWikiProposal,
    handleForget,
    handleInitForgeWiki,
    handlePin,
    saveDraft,
    startEdit,
  };
}
