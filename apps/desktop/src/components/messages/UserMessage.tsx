import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { MarkdownRenderer } from "@/components/messages/MarkdownRenderer";
import { MessageCopyAction } from "@/components/messages/MessageCopyAction";

export function UserMessage({ block }: { block: BlockState }) {
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const isLongMessage = block.content.length > 160 || block.content.includes("\n");

  return (
    <div className="forge-user-message-row" data-message-length={isLongMessage ? "long" : "short"}>
      <div
        data-testid="user-message"
        data-message-role="user"
        data-long={isLongMessage ? "true" : "false"}
        className="forge-message-with-actions user-command-note"
      >
        <MessageCopyAction text={block.content} label="提问" />
        <div className="markdown-content">
          <MarkdownRenderer content={block.content} onOpenFileRef={setPreviewFileRef} />
        </div>
      </div>
      <FilePreviewSheet fileRef={previewFileRef} onClose={() => setPreviewFileRef(null)} />
    </div>
  );
}
