import type { WikiMemory } from "@/lib/protocol";
import { ForgeSurface } from "@/components/primitives/surface";
import type { DraftState } from "./WikiSectionTypes";
import { EmptyState, SectionHeader } from "./WikiSectionChrome";
import { MemoryRow } from "./WikiRecordRows";

export function SavedBackgroundSection({
  projectMemories,
  draft,
  busyId,
  onDraftChange,
  onEditMemory,
  onSaveDraft,
  onCancelDraft,
  onPinMemory,
  onForgetMemory,
}: {
  projectMemories: WikiMemory[];
  draft: DraftState | null;
  busyId: string | null;
  onDraftChange: (draft: DraftState | null) => void;
  onEditMemory: (memory: WikiMemory) => void;
  onSaveDraft: () => void;
  onCancelDraft: () => void;
  onPinMemory: (memoryId: string) => void;
  onForgetMemory: (memoryId: string) => void;
}) {
  return (
    <section>
      <SectionHeader title="已保存背景" meta={projectMemories.length > 0 ? `${projectMemories.length} 条` : null} />
      <ForgeSurface className="overflow-hidden">
        {projectMemories.length === 0 ? (
          <EmptyState label="还没有已保存背景" />
        ) : (
          <div className="divide-y divide-border">
            {projectMemories.map((memory) => (
              <MemoryRow
                key={memory.id}
                memory={memory}
                draft={draft?.memoryId === memory.id ? draft : null}
                busy={busyId === memory.id}
                onDraftChange={onDraftChange}
                onEdit={() => onEditMemory(memory)}
                onSave={onSaveDraft}
                onCancel={onCancelDraft}
                onPin={memory.status === "pinned" ? undefined : () => onPinMemory(memory.id)}
                onForget={() => onForgetMemory(memory.id)}
              />
            ))}
          </div>
        )}
      </ForgeSurface>
    </section>
  );
}
