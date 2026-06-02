import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getForgeWikiState,
  listMemories,
} from "@/lib/tauri";
import type { ForgeWikiState } from "@/lib/protocol";
import { useStore } from "@/store";
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
  const [forgeWikiState, setForgeWikiState] = useState<ForgeWikiState | null>(null);
  const requestIdRef = useRef(0);

  const currentProjectPath = useMemo(() => normalizeProjectPath(projectPath), [projectPath]);
  const isCurrentRequest = useCurrentWikiRequest(currentProjectPath, sessionId);

  const refresh = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    if (!currentProjectPath) {
      setLoading(false);
      setError("");
      setForgeWikiState(null);
      return;
    }

    setLoading(true);
    setError("");
    try {
      const [nextMemories, nextForgeWikiState] = await Promise.all([
        listMemories(undefined, currentProjectPath, sessionId),
        getForgeWikiState(currentProjectPath, sessionId),
      ]);
      if (requestIdRef.current === requestId) {
        setMemories(nextMemories);
        setForgeWikiState(nextForgeWikiState);
      }
    } catch (err) {
      if (requestIdRef.current === requestId) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (requestIdRef.current === requestId) setLoading(false);
    }
  }, [currentProjectPath, setMemories]);

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
    setForgeWikiState,
    upsertForgeWikiProposal,
  });

  return (
    <WikiSectionsView
      currentProjectPath={currentProjectPath}
      forgeWikiState={forgeWikiState}
      loading={loading}
      error={error}
      draft={draft}
      busyId={busyId}
      candidateMemories={candidateMemories}
      projectMemories={projectMemories}
      visibleForgeWikiProposals={visibleForgeWikiProposals}
      pendingForgeWikiProposals={pendingForgeWikiProposals}
      onRefresh={refresh}
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
