import { useState, useEffect, useRef } from "react";
import type { BlockState } from "@/lib/protocol";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { MarkdownRenderer } from "@/components/messages/MarkdownRenderer";
import { MessageCopyAction } from "@/components/messages/MessageCopyAction";
import { ProcessStatusDots } from "@/components/messages/ProcessStatusDots";

const STREAM_THROTTLE_MS = 96;

export function TextBlock({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  if (!block.content && block.isComplete) return null;
  const hasContent = Boolean(block.content);

  // Throttle streaming text so the rendered markdown surface stays visually stable.
  const [displayContent, setDisplayContent] = useState(block.content);
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastUpdateRef = useRef(0);

  useEffect(() => {
    if (block.isComplete) {
      // Immediately show final content on completion
      if (timerRef.current) clearTimeout(timerRef.current);
      setDisplayContent(block.content);
      return;
    }
    // During streaming, throttle to every STREAM_THROTTLE_MS
    const now = performance.now();
    const elapsed = now - lastUpdateRef.current;
    if (elapsed >= STREAM_THROTTLE_MS) {
      lastUpdateRef.current = now;
      setDisplayContent(block.content);
    } else {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        lastUpdateRef.current = performance.now();
        setDisplayContent(block.content);
      }, STREAM_THROTTLE_MS - elapsed);
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [block.content, block.isComplete]);

  const renderedContent = block.isComplete ? block.content : displayContent;

  return (
    <div>
      {hasContent ? (
        <div
          data-testid="assistant-message"
          data-message-role="assistant"
          data-state={block.isComplete ? "complete" : "streaming"}
          className="forge-message-with-actions assistant-paper"
        >
          <span aria-hidden="true" className="forge-assistant-avatar">F</span>
          <span className="forge-assistant-name">Forge</span>
          <MessageCopyAction text={block.content} label="回复" />
          <div className="markdown-content">
            <MarkdownRenderer
              content={renderedContent}
              onOpenFileRef={setPreviewFileRef}
              streaming={!block.isComplete}
              showSectionIndex
            />
          </div>
        </div>
      ) : (
        <div
          data-testid="assistant-streaming-status"
          data-state="running"
          role="status"
          aria-live="polite"
          className="forge-status-row"
        >
          <ProcessStatusDots testId="assistant-streaming-dots" />
          <span>正在组织回复</span>
        </div>
      )}
      <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
    </div>
  );
}
