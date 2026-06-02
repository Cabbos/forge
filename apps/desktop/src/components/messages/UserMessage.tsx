import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { FilePreviewSheet } from "@/components/messages/FilePreviewSheet";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { MarkdownRenderer } from "@/components/messages/MarkdownRenderer";
import { MessageCopyAction } from "@/components/messages/MessageCopyAction";

export function UserMessage({ block }: { block: BlockState }) {
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);

  return (
    <div className="forge-user-message-row">
      <div
        data-testid="user-message"
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
