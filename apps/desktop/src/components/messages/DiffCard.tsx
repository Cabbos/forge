import { FileDiff } from "lucide-react";
import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { useStore } from "@/store";
import { deriveDiffView } from "@/components/messages/diffPresentation";
import { DiffBody } from "@/components/messages/DiffBody";
import { DiffHeaderActions } from "@/components/messages/DiffHeaderActions";

export function DiffCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const workingDir = useStore((s) => sessionId ? s.sessions.get(sessionId)?.workingDir ?? null : null);
  const [expanded, setExpanded] = useState(false);
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const filePath = (block.metadata.file_path as string) || "";
  const diff = block.content || "";

  if (!diff) return null;

  const view = deriveDiffView(diff, expanded);

  return (
    <div data-testid="diff-card">
      <MessagePanel className="forge-diff-card">
        <MessagePanelHeader
          icon={<ForgeIcon icon={FileDiff} tone="context" contained={false} className="size-3.5" />}
          title="文件改动"
          meta={(
            <div className="flex min-w-0 items-center gap-2">
                <span data-testid="diff-file-path" className="truncate font-mono">{filePath || "未命名文件"}</span>
              <span data-testid="diff-stat" className="shrink-0 font-mono">
                <span className="forge-diff-stat-add">+{view.additions}</span>
                <span className="mx-1">/</span>
                <span className="forge-diff-stat-remove">-{view.deletions}</span>
              </span>
            </div>
          )}
          actions={(
            <DiffHeaderActions
              diff={diff}
              filePath={filePath}
              firstChangedLine={view.firstChangedLine}
              sessionId={sessionId}
              workingDir={workingDir}
              onPreviewFile={setPreviewFileRef}
            />
          )}
        />
        <div data-testid="diff-summary" className="forge-diff-summary">
          <span>{view.hunkCount} 个变更块</span>
          {view.firstChangedLine ? <span>首处第 {view.firstChangedLine} 行</span> : null}
          <span>{view.lines.length} 行</span>
        </div>
        <DiffBody
          visibleLines={view.visibleLines}
          isLongDiff={view.isLongDiff}
          expanded={expanded}
          hiddenLineCount={view.hiddenLineCount}
          onToggleExpanded={() => setExpanded((current) => !current)}
        />
        <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
      </MessagePanel>
    </div>
  );
}
