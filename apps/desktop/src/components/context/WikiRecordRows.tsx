import { useState } from "react";
import { Check, ChevronDown, Edit3, Pin, Trash2, X } from "lucide-react";
import type {
  ForgeWikiUpdateProposal,
  WikiMemory,
} from "@/lib/protocol";
import type { DraftState } from "./WikiSectionTypes";
import { IconButton, RowIntentLabel } from "./WikiSectionChrome";
import { MemoryDraftEditor } from "./WikiMemoryDraftEditor";
import {
  categoryLabel,
  proposalStatusLabel,
  proposalStatusMeta,
  statusLabel,
} from "./WikiRecordLabels";

export function ForgeWikiProposalRow({
  proposal,
  busy,
  onAccept,
  onDiscard,
}: {
  proposal: ForgeWikiUpdateProposal;
  busy: boolean;
  onAccept: () => void;
  onDiscard: () => void;
}) {
  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <RowIntentLabel>{proposalStatusLabel(proposal.status)}</RowIntentLabel>
          <div className="truncate text-xs font-medium text-foreground">{proposal.title}</div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {proposal.summary}
          </div>
          {proposal.patch_preview ? (
            <div className="mt-2 rounded-md border border-border bg-background/50 px-2 py-2">
              <div className="text-[10px] font-medium leading-none text-muted-foreground">
                写入预览
              </div>
              <div className="mt-1 max-h-20 overflow-hidden whitespace-pre-wrap break-words font-mono text-[10px] leading-relaxed text-muted-foreground">
                {proposal.patch_preview}
              </div>
            </div>
          ) : null}
          <RecordMetaGrid
            rows={[
              ["保存位置", "项目记录"],
              ["项目记录页面", proposal.target_pages.join(", ")],
              ["确认后状态", proposalStatusMeta(proposal.status)],
            ]}
          />
        </div>
        {proposal.status === "pending" && (
          <div className="flex shrink-0 gap-0.5">
            <IconButton title="接受" onClick={onAccept} disabled={busy}>
              <Check className="size-3" />
            </IconButton>
            <IconButton title="丢弃" onClick={onDiscard} disabled={busy}>
              <X className="size-3" />
            </IconButton>
          </div>
        )}
      </div>
    </div>
  );
}

export function MemoryRow({
  memory,
  draft,
  busy,
  intentLabel,
  onDraftChange,
  onEdit,
  onSave,
  onCancel,
  onAccept,
  onPin,
  onForget,
}: {
  memory: WikiMemory;
  draft: DraftState | null;
  busy: boolean;
  intentLabel?: string;
  onDraftChange: (draft: DraftState | null) => void;
  onEdit: () => void;
  onSave: () => void;
  onCancel: () => void;
  onAccept?: () => void;
  onPin?: () => void;
  onForget: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = memory.title.length > 48 || memory.body.length > 140;

  if (draft) {
    return (
      <MemoryDraftEditor
        draft={draft}
        busy={busy}
        onDraftChange={onDraftChange}
        onSave={onSave}
        onCancel={onCancel}
      />
    );
  }

  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          {intentLabel && <RowIntentLabel>{intentLabel}</RowIntentLabel>}
          <div className={`${expanded ? "break-words" : "truncate"} text-xs font-medium text-foreground`}>
            {memory.title}
          </div>
          <div
            className={`mt-1 break-words text-[11px] leading-relaxed text-muted-foreground ${
              expanded ? "whitespace-pre-wrap" : "max-h-[3.8rem] overflow-hidden"
            }`}
          >
            {memory.body}
          </div>
          {canExpand && (
            <button
              type="button"
              className="mt-1 inline-flex items-center gap-1 text-[10px] font-medium text-muted-foreground hover:text-foreground"
              onClick={() => setExpanded((value) => !value)}
            >
              <ChevronDown className={`size-3 transition-transform ${expanded ? "rotate-180" : ""}`} />
              {expanded ? "收起详情" : "展开详情"}
            </button>
          )}
          {intentLabel && (
            <RecordMetaGrid
              rows={[
                ["保存位置", "已保存背景"],
                ["建议原因", categoryLabel(memory.category)],
                ["确认后状态", "已确认"],
              ]}
            />
          )}
        </div>
        <div className="flex shrink-0 gap-0.5">
          {onAccept && (
            <IconButton title="接受" onClick={onAccept} disabled={busy}>
              <Check className="size-3" />
            </IconButton>
          )}
          <IconButton title="编辑" onClick={onEdit} disabled={busy}>
            <Edit3 className="size-3" />
          </IconButton>
          {onPin && (
            <IconButton title="置顶" onClick={onPin} disabled={busy}>
              <Pin className="size-3" />
            </IconButton>
          )}
          <IconButton title="忘记" onClick={onForget} disabled={busy}>
            <Trash2 className="size-3" />
          </IconButton>
        </div>
      </div>
      <div className="mt-2 grid grid-cols-[minmax(0,1fr)_58px_48px] gap-2 text-[10px] text-muted-foreground/70">
        <span className="truncate">{categoryLabel(memory.category)}</span>
        <span className="truncate text-right">{statusLabel(memory.status)}</span>
        <span className="text-right font-mono">{Math.round(memory.confidence * 100)}%</span>
      </div>
    </div>
  );
}

function RecordMetaGrid({ rows }: { rows: Array<[string, string]> }) {
  return (
    <dl className="mt-2 space-y-1 text-[10px] leading-relaxed text-muted-foreground/75">
      {rows.map(([label, value]) => (
        <div key={label} className="grid grid-cols-[64px_minmax(0,1fr)] gap-2">
          <dt className="text-muted-foreground">{label}</dt>
          <dd className="min-w-0 break-words">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
