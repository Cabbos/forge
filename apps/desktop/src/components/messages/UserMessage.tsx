import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { FilePreviewSheet, type FileRef } from "@/components/messages/FilePreviewSheet";
import { MessageCopyAction } from "@/components/messages/MessageCopyAction";
import { MarkdownRenderer } from "@/components/messages/TextBlock";

export function UserMessage({ block }: { block: BlockState }) {
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const isLong = block.content.length > 640 || block.content.split("\n").length > 8;

  return (
    <div className="flex justify-end">
      <div
        data-testid="user-message"
        data-long={isLong ? "true" : "false"}
        className="forge-message-with-actions forge-user-message"
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
