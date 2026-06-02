import type { ForgeWikiUpdateProposal, WikiMemory } from "@/lib/protocol";
import { ForgeSurface } from "@/components/primitives/surface";
import type { DraftState } from "./WikiSectionTypes";
import { EmptyState, SectionHeader } from "./WikiSectionChrome";
import { ForgeWikiProposalRow, MemoryRow } from "./WikiRecordRows";

export function PendingUpdatesSection({
  candidateMemories,
  visibleForgeWikiProposals,
  pendingForgeWikiProposals,
  draft,
  busyId,
  onDraftChange,
  onEditMemory,
  onSaveDraft,
  onCancelDraft,
  onAcceptMemory,
  onPinMemory,
  onForgetMemory,
  onAcceptForgeWikiProposal,
  onDiscardForgeWikiProposal,
}: {
  candidateMemories: WikiMemory[];
  visibleForgeWikiProposals: ForgeWikiUpdateProposal[];
  pendingForgeWikiProposals: ForgeWikiUpdateProposal[];
  draft: DraftState | null;
  busyId: string | null;
  onDraftChange: (draft: DraftState | null) => void;
  onEditMemory: (memory: WikiMemory) => void;
  onSaveDraft: () => void;
  onCancelDraft: () => void;
  onAcceptMemory: (memoryId: string) => void;
  onPinMemory: (memoryId: string) => void;
  onForgetMemory: (memoryId: string) => void;
  onAcceptForgeWikiProposal: (proposal: ForgeWikiUpdateProposal) => void;
  onDiscardForgeWikiProposal: (proposal: ForgeWikiUpdateProposal) => void;
}) {
  return (
    <section>
      <SectionHeader
        title="建议更新记录"
        meta={
          pendingForgeWikiProposals.length + candidateMemories.length > 0
            ? `${pendingForgeWikiProposals.length + candidateMemories.length} 条`
            : null
        }
      />
      <p className="-mt-1 mb-2 text-[10px] leading-relaxed text-muted-foreground/70">
        确认后会进入项目记录或已保存背景
      </p>
      <ForgeSurface className="overflow-hidden">
        {visibleForgeWikiProposals.length === 0 && candidateMemories.length === 0 ? (
          <EmptyState label="没有待确认的记录更新" />
        ) : (
          <div className="divide-y divide-border">
            {candidateMemories.map((memory) => (
              <MemoryRow
                key={memory.id}
                memory={memory}
                draft={draft?.memoryId === memory.id ? draft : null}
                busy={busyId === memory.id}
                intentLabel="建议保存为已保存背景"
                onDraftChange={onDraftChange}
                onEdit={() => onEditMemory(memory)}
                onSave={onSaveDraft}
                onCancel={onCancelDraft}
                onAccept={() => onAcceptMemory(memory.id)}
                onPin={() => onPinMemory(memory.id)}
                onForget={() => onForgetMemory(memory.id)}
              />
            ))}
            {visibleForgeWikiProposals.map((proposal) => (
              <ForgeWikiProposalRow
                key={proposal.id}
                proposal={proposal}
                busy={busyId === proposal.id}
                onAccept={() => onAcceptForgeWikiProposal(proposal)}
                onDiscard={() => onDiscardForgeWikiProposal(proposal)}
              />
            ))}
          </div>
        )}
      </ForgeSurface>
    </section>
  );
}
