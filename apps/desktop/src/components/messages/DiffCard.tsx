import { ChevronRight, FileDiff } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { useStore } from "@/store";
import { deriveDiffView } from "@/components/messages/diffPresentation";
import { DiffBody } from "@/components/messages/DiffBody";
import { DiffHeaderActions } from "@/components/messages/DiffHeaderActions";
import { cn } from "@/lib/utils";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

export function DiffCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const selectWorkingDir = useCallback(
    (s: ReturnType<typeof useStore.getState>) => sessionId ? s.sessions.get(sessionId)?.workingDir ?? null : null,
    [sessionId],
  );
  const workingDir = useStore(selectWorkingDir);
  const rootRef = useRef<HTMLDivElement>(null);
  const [bodyOpen, setBodyOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const filePath = (block.metadata.file_path as string) || "";
  const diff = block.content || "";

  if (!diff) return null;

  const view = deriveDiffView(diff, expanded);
  const toggleBody = () => {
    setBodyOpen((current) => {
      if (current) setExpanded(false);
      return !current;
    });
  };

  useGSAP(() => {
    if (!bodyOpen || prefersReducedMotion()) return;
    const body = rootRef.current?.querySelector<HTMLElement>("[data-forge-motion='diff-body']");
    if (!body) return;

    gsap.fromTo(
      body,
      { autoAlpha: 0, y: -5 },
      {
        autoAlpha: 1,
        y: 0,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: rootRef, dependencies: [bodyOpen] });

  return (
    <div ref={rootRef} data-testid="diff-card">
      <MessagePanel className="forge-diff-card" data-diff-open={bodyOpen ? "true" : "false"}>
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
          <button
            type="button"
            data-testid="diff-body-toggle"
            aria-expanded={bodyOpen}
            onClick={toggleBody}
            className="forge-diff-toggle"
          >
            <ChevronRight className={cn("size-3 transition-transform", bodyOpen && "rotate-90")} />
            {bodyOpen ? "隐藏改动" : "查看改动"}
          </button>
        </div>
        {bodyOpen && (
          <div data-forge-motion="diff-body">
            <DiffBody
              visibleLines={view.visibleLines}
              isLongDiff={view.isLongDiff}
              expanded={expanded}
              hiddenLineCount={view.hiddenLineCount}
              onToggleExpanded={() => setExpanded((current) => !current)}
            />
          </div>
        )}
        <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
      </MessagePanel>
    </div>
  );
}
