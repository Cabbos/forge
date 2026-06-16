import { ChevronRight, FileDiff } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import type { BlockState } from "@/lib/protocol";
import { ForgeIcon } from "@/components/primitives/icon";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { MessagePanel } from "@/components/messages/MessagePanel";
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
    <div ref={rootRef} data-testid="diff-card" className="diff-filmstrip">
      <MessagePanel className="forge-diff-card" data-diff-open={bodyOpen ? "true" : "false"}>
        <div className="diff-filmstrip-header">
          <ForgeIcon icon={FileDiff} tone="context" contained={false} className="size-3.5" />
          <span className="diff-filmstrip-title">文件改动</span>
          <span data-testid="diff-file-path" className="diff-filmstrip-file truncate font-mono">
            {filePath || "未命名文件"}
          </span>
          <span data-testid="diff-stat" className="diff-filmstrip-stat shrink-0 font-mono">
            <span className="forge-diff-stat-add">+{view.additions}</span>
            <span className="diff-filmstrip-separator">/</span>
            <span className="forge-diff-stat-remove">-{view.deletions}</span>
          </span>
          <div className="diff-filmstrip-actions">
            <DiffHeaderActions
              diff={diff}
              filePath={filePath}
              firstChangedLine={view.firstChangedLine}
              sessionId={sessionId}
              workingDir={workingDir}
              onPreviewFile={setPreviewFileRef}
            />
          </div>
        </div>
        <div data-testid="diff-summary" className="forge-diff-summary">
          {view.fileCount > 1 ? <span>{view.fileCount} 个文件</span> : null}
          <span>{view.hunkCount} 个变更块</span>
          {view.firstChangedLine ? <span>首处第 {view.firstChangedLine} 行</span> : null}
          <span>{view.lines.length} 行</span>
          <ButtonPrimitive
            type="button"
            data-testid="diff-body-toggle"
            aria-expanded={bodyOpen}
            onClick={toggleBody}
            className="forge-diff-toggle"
          >
            <ChevronRight className={cn("size-3 transition-transform", bodyOpen && "rotate-90")} />
            {bodyOpen ? "隐藏改动" : "查看改动"}
          </ButtonPrimitive>
        </div>
        {view.fileCount > 1 && (
          <div data-testid="diff-file-tree" className="forge-diff-file-tree">
            {view.visibleFiles.map((file) => (
              <span key={file.path} className="forge-diff-file-chip" data-status={file.status}>
                <span className="forge-diff-file-path">{file.path}</span>
                <span className="forge-diff-file-stat">
                  +{file.additions}/-{file.deletions}
                </span>
              </span>
            ))}
            {view.hiddenFileCount > 0 && (
              <span className="forge-diff-file-chip forge-diff-file-chip-more">
                +{view.hiddenFileCount} 文件
              </span>
            )}
          </div>
        )}
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
      </MessagePanel>
      <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
    </div>
  );
}
