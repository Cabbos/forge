import { useState } from "react";
import { Copy, ExternalLink } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import { openFile } from "@/lib/tauri";

interface FilePreviewTarget {
  path: string;
  line?: number;
}

interface FilePreviewActionsProps {
  copyText: string;
  fileRef: FilePreviewTarget | null;
  sessionId?: string;
  workingDir: string | null;
  onError: (message: string) => void;
}

export function FilePreviewActions({
  copyText,
  fileRef,
  sessionId,
  workingDir,
  onError,
}: FilePreviewActionsProps) {
  const [copied, setCopied] = useState(false);
  const copyLabel = copied ? "已复制" : "复制路径";

  const copyPath = async () => {
    if (!copyText) return;
    await navigator.clipboard?.writeText(copyText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  const openExternally = () => {
    if (!fileRef) return;
    openFile(fileRef.path, fileRef.line, sessionId, workingDir).catch((err) => onError(String(err)));
  };

  return (
    <div className="flex items-center justify-between border-t border-border bg-popover p-3">
      <ForgeButton variant="outline" size="sm" onClick={copyPath} disabled={!copyText}>
        <Copy className="size-3.5" />
        {copyLabel}
      </ForgeButton>
      <ForgeButton variant="secondary" size="sm" onClick={openExternally} disabled={!fileRef}>
        <ExternalLink className="size-3.5" />
        在编辑器打开
      </ForgeButton>
    </div>
  );
}
