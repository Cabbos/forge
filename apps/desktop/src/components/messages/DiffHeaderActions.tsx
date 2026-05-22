import { Check, Copy, ExternalLink, LocateFixed } from "lucide-react";
import { useState } from "react";
import { openFile } from "@/lib/tauri";
import type { FileRef } from "@/components/messages/filePreviewTypes";

interface DiffHeaderActionsProps {
  diff: string;
  filePath: string;
  firstChangedLine?: number;
  sessionId?: string;
  workingDir: string | null;
  onPreviewFile: (fileRef: FileRef) => void;
}

export function DiffHeaderActions({
  diff,
  filePath,
  firstChangedLine,
  sessionId,
  workingDir,
  onPreviewFile,
}: DiffHeaderActionsProps) {
  const [copied, setCopied] = useState(false);

  const copyDiff = async () => {
    await navigator.clipboard?.writeText(diff);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  const openChangedLine = () => {
    if (!filePath) return;
    onPreviewFile({ path: filePath, line: firstChangedLine });
  };

  const openInEditor = () => {
    if (!filePath) return;
    openFile(filePath, undefined, sessionId, workingDir).catch(() => {});
  };

  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        aria-label={copied ? "已复制 diff" : "复制 diff"}
        title={copied ? "已复制" : "复制 diff"}
        onClick={copyDiff}
        className="forge-icon-button size-6"
      >
        {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
      </button>
      <button
        type="button"
        aria-label="打开文件"
        title="打开文件"
        onClick={openInEditor}
        disabled={!filePath}
        className="forge-icon-button size-6 disabled:cursor-default disabled:opacity-45"
      >
        <ExternalLink className="size-3" />
      </button>
      <button
        type="button"
        aria-label="定位首处改动"
        title="定位首处改动"
        onClick={openChangedLine}
        disabled={!filePath || !firstChangedLine}
        className="forge-icon-button size-6 disabled:cursor-default disabled:opacity-45"
      >
        <LocateFixed className="size-3" />
      </button>
    </div>
  );
}
