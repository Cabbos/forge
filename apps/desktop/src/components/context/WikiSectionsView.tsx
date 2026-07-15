import type { ForgeWikiState, ForgeWikiUpdateProposal, WikiMemory } from "@/lib/protocol";
import type { DraftState } from "./WikiSectionTypes";
import { PendingUpdatesSection } from "./WikiPendingUpdatesSection";
import { ProjectRecordsSection } from "./WikiProjectRecordsSection";
import { SavedBackgroundSection } from "./WikiSavedBackgroundSection";
import { ContinuityExperiencesSection } from "./ContinuityExperiencesSection";
import { UnifiedMemorySection } from "./UnifiedMemorySection";

interface WikiSectionsViewProps {
  currentProjectPath: string;
  sessionId: string | null;
  forgeWikiState: ForgeWikiState | null;
  loading: boolean;
  error: string;
  draft: DraftState | null;
  busyId: string | null;
  candidateMemories: WikiMemory[];
  projectMemories: WikiMemory[];
  visibleForgeWikiProposals: ForgeWikiUpdateProposal[];
  pendingForgeWikiProposals: ForgeWikiUpdateProposal[];
  onRefresh: () => void;
  onInitForgeWiki: () => void;
  onDraftChange: (draft: DraftState | null) => void;
  onEditMemory: (memory: WikiMemory) => void;
  onSaveDraft: () => void;
  onCancelDraft: () => void;
  onAcceptMemory: (memoryId: string) => void;
  onPinMemory: (memoryId: string) => void;
  onForgetMemory: (memoryId: string) => void;
  onAcceptForgeWikiProposal: (proposal: ForgeWikiUpdateProposal) => void;
  onDiscardForgeWikiProposal: (proposal: ForgeWikiUpdateProposal) => void;
}

export function WikiSectionsView({
  currentProjectPath,
  sessionId,
  forgeWikiState,
  loading,
  error,
  draft,
  busyId,
  candidateMemories,
  projectMemories,
  visibleForgeWikiProposals,
  pendingForgeWikiProposals,
  onRefresh,
  onInitForgeWiki,
  onDraftChange,
  onEditMemory,
  onSaveDraft,
  onCancelDraft,
  onAcceptMemory,
  onPinMemory,
  onForgetMemory,
  onAcceptForgeWikiProposal,
  onDiscardForgeWikiProposal,
}: WikiSectionsViewProps) {
  return (
    <>
      <ProjectRecordsSection
        currentProjectPath={currentProjectPath}
        forgeWikiState={forgeWikiState}
        loading={loading}
        busyId={busyId}
        onRefresh={onRefresh}
        onInitForgeWiki={onInitForgeWiki}
      />

      <UnifiedMemorySection currentProjectPath={currentProjectPath} sessionId={sessionId} />

      <PendingUpdatesSection
        candidateMemories={candidateMemories}
        visibleForgeWikiProposals={visibleForgeWikiProposals}
        pendingForgeWikiProposals={pendingForgeWikiProposals}
        draft={draft}
        busyId={busyId}
        onDraftChange={onDraftChange}
        onEditMemory={onEditMemory}
        onSaveDraft={onSaveDraft}
        onCancelDraft={onCancelDraft}
        onAcceptMemory={onAcceptMemory}
        onPinMemory={onPinMemory}
        onForgetMemory={onForgetMemory}
        onAcceptForgeWikiProposal={onAcceptForgeWikiProposal}
        onDiscardForgeWikiProposal={onDiscardForgeWikiProposal}
      />

      <SavedBackgroundSection
        projectMemories={projectMemories}
        draft={draft}
        busyId={busyId}
        onDraftChange={onDraftChange}
        onEditMemory={onEditMemory}
        onSaveDraft={onSaveDraft}
        onCancelDraft={onCancelDraft}
        onPinMemory={onPinMemory}
        onForgetMemory={onForgetMemory}
      />

      <ContinuityExperiencesSection currentProjectPath={currentProjectPath} sessionId={sessionId} />

      {error && (
        <div className="rounded-md border border-destructive/20 bg-destructive/5 px-2 py-1.5 text-[11px] leading-relaxed text-destructive">
          {error}
        </div>
      )}
    </>
  );
}
