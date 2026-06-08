import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { listMemories } from "@/lib/tauri";
import { useStore } from "@/store";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useForgeWikiStateQuery } from "@/hooks/queries/useForgeWikiStateQuery";
import {
  EMPTY_FORGE_WIKI_PROPOSALS,
  filterCandidateMemories,
  filterProjectMemories,
  filterVisibleForgeWikiProposals,
  indexMemoriesById,
  normalizeProjectPath,
} from "./WikiSectionsModel";
import { WikiSectionsView } from "./WikiSectionsView";
import type { DraftState } from "./WikiSectionTypes";
import { useCurrentWikiRequest } from "./useCurrentWikiRequest";
import { useWikiBusyState } from "./useWikiBusyState";
import { useWikiSectionsActions } from "./useWikiSectionsActions";

interface WikiSectionsProps {
  sessionId: string | null;
  projectPath: string | null;
}

export function WikiSections({ sessionId, projectPath }: WikiSectionsProps) {
  const queryClient = useQueryClient();
  const memories = useStore((s) => s.memories);
  const forgeWikiProposals = useStore((s) =>
    sessionId ? s.forgeWikiProposalsBySession.get(sessionId) ?? EMPTY_FORGE_WIKI_PROPOSALS : EMPTY_FORGE_WIKI_PROPOSALS,
  );
  const setMemories = useStore((s) => s.setMemories);
  const upsertForgeWikiProposal = useStore((s) => s.upsertForgeWikiProposal);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [draft, setDraft] = useState<DraftState | null>(null);
  const { busyId, setBusyId, beginBusy, clearBusy } = useWikiBusyState();
  const requestIdRef = useRef(0);

  const currentProjectPath = useMemo(() => normalizeProjectPath(projectPath), [projectPath]);
  const isCurrentRequest = useCurrentWikiRequest(currentProjectPath, sessionId);

  const {
    data: forgeWikiState = null,
    isFetching: wikiStateFetching,
    isError: wikiStateIsError,
    error: wikiStateError,
  } = useForgeWikiStateQuery(currentProjectPath, sessionId, !!currentProjectPath);

  const queryError = getQueryErrorMessage(wikiStateIsError ? wikiStateError : null);
  const displayError = error || queryError;

  const refresh = useCallback(async (options?: { refreshWikiState?: boolean }) => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    if (!currentProjectPath) {
      setLoading(false);
      setError("");
      return;
    }

    setLoading(true);
    setError("");
    try {
      if (options?.refreshWikiState) {
        await queryClient.invalidateQueries({ queryKey: queryKeys.forgeWikiState(currentProjectPath, sessionId) });
      }
      const nextMemories = await listMemories(undefined, currentProjectPath, sessionId);
      if (requestIdRef.current === requestId && isCurrentRequest(currentProjectPath, sessionId)) {
        setMemories(nextMemories);
      }
    } catch (err) {
      if (requestIdRef.current === requestId && isCurrentRequest(currentProjectPath, sessionId)) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (requestIdRef.current === requestId && isCurrentRequest(currentProjectPath, sessionId)) setLoading(false);
    }
  }, [currentProjectPath, isCurrentRequest, queryClient, sessionId, setMemories]);

  const handleManualRefresh = useCallback(() => {
    void refresh({ refreshWikiState: true });
  }, [refresh]);

  useEffect(() => {
    refresh();
    return () => {
      requestIdRef.current += 1;
    };
  }, [refresh]);

  const memoriesById = useMemo(
    () => indexMemoriesById(memories, currentProjectPath),
    [currentProjectPath, memories],
  );

  const candidateMemories = useMemo(
    () => filterCandidateMemories(memories, currentProjectPath),
    [currentProjectPath, memories],
  );

  const projectMemories = useMemo(
    () => filterProjectMemories(memories, currentProjectPath),
    [currentProjectPath, memories],
  );

  const visibleForgeWikiProposals = useMemo(
    () => filterVisibleForgeWikiProposals(forgeWikiProposals, currentProjectPath),
    [currentProjectPath, forgeWikiProposals],
  );

  const pendingForgeWikiProposals = useMemo(
    () => visibleForgeWikiProposals.filter((proposal) => proposal.status === "pending"),
    [visibleForgeWikiProposals],
  );

  const {
    handleAccept,
    handleAcceptForgeWikiProposal,
    handleDiscardForgeWikiProposal,
    handleForget,
    handleInitForgeWiki,
    handlePin,
    saveDraft,
    startEdit,
  } = useWikiSectionsActions({
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
  });

  return (
    <WikiSectionsView
      currentProjectPath={currentProjectPath}
      sessionId={sessionId}
      forgeWikiState={forgeWikiState}
      loading={loading || wikiStateFetching}
      error={displayError}
      draft={draft}
      busyId={busyId}
      candidateMemories={candidateMemories}
      projectMemories={projectMemories}
      visibleForgeWikiProposals={visibleForgeWikiProposals}
      pendingForgeWikiProposals={pendingForgeWikiProposals}
      onRefresh={handleManualRefresh}
      onInitForgeWiki={handleInitForgeWiki}
      onDraftChange={setDraft}
      onEditMemory={startEdit}
      onSaveDraft={saveDraft}
      onCancelDraft={() => setDraft(null)}
      onAcceptMemory={handleAccept}
      onPinMemory={handlePin}
      onForgetMemory={handleForget}
      onAcceptForgeWikiProposal={handleAcceptForgeWikiProposal}
      onDiscardForgeWikiProposal={handleDiscardForgeWikiProposal}
    />
  );
}
