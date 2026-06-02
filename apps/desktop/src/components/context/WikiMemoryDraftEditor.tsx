import { Check, X } from "lucide-react";
import type { DraftState } from "./WikiSectionTypes";
import { IconButton } from "./WikiSectionChrome";

export function MemoryDraftEditor({
  draft,
  busy,
  onDraftChange,
  onSave,
  onCancel,
}: {
  draft: DraftState;
  busy: boolean;
  onDraftChange: (draft: DraftState | null) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="space-y-2 px-3 py-2.5">
      <input
        value={draft.title}
        onChange={(event) => onDraftChange({ ...draft, title: event.target.value })}
        className="w-full rounded border border-border bg-background/70 px-2 py-1 text-xs text-foreground outline-none focus:border-primary/50"
      />
      <textarea
        value={draft.body}
        onChange={(event) => onDraftChange({ ...draft, body: event.target.value })}
        rows={3}
        className="max-h-24 w-full resize-none rounded border border-border bg-background/70 px-2 py-1 text-[11px] leading-relaxed text-foreground outline-none focus:border-primary/50 break-words"
      />
      <div className="flex justify-end gap-1">
        <IconButton title="取消" onClick={onCancel} disabled={busy}>
          <X className="size-3" />
        </IconButton>
        <IconButton title="保存" onClick={onSave} disabled={busy}>
          <Check className="size-3" />
        </IconButton>
      </div>
    </div>
  );
}
