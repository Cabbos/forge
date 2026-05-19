import { Check, ChevronDown, Copy, ExternalLink, FileDiff, LocateFixed } from "lucide-react";
import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { FilePreviewSheet, type FileRef } from "@/components/messages/FilePreviewSheet";
import { openFile } from "@/lib/tauri";

const INITIAL_VISIBLE_DIFF_LINES = 28;

type DiffLineType = "header" | "hunk" | "add" | "remove" | "context";

interface ParsedDiffLine {
  raw: string;
  type: DiffLineType;
  oldNumber: number | null;
  newNumber: number | null;
}

const DIFF_LINE_CLASS: Record<DiffLineType, string> = {
  add: "forge-diff-line forge-diff-line-added",
  remove: "forge-diff-line forge-diff-line-removed",
  hunk: "forge-diff-line forge-diff-line-hunk",
  header: "forge-diff-line forge-diff-line-header",
  context: "forge-diff-line forge-diff-line-context",
};

export function DiffCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const [copied, setCopied] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const filePath = (block.metadata.file_path as string) || "";
  const diff = block.content || "";

  if (!diff) return null;

  const parsed = parseDiff(diff);
  const isLongDiff = parsed.lines.length > INITIAL_VISIBLE_DIFF_LINES;
  const visibleLines = expanded ? parsed.lines : parsed.lines.slice(0, INITIAL_VISIBLE_DIFF_LINES);
  const hiddenLineCount = Math.max(0, parsed.lines.length - visibleLines.length);
  const firstChangedLine = parsed.firstChangedLine ?? undefined;
  const copyDiff = async () => {
    await navigator.clipboard?.writeText(diff);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };
  const openChangedLine = () => {
    if (!filePath) return;
    setPreviewFileRef({ path: filePath, line: firstChangedLine });
  };
  const openInEditor = () => {
    if (!filePath) return;
    openFile(filePath, undefined, sessionId).catch(() => {});
  };

  return (
    <div data-testid="diff-card">
      <MessagePanel className="forge-diff-card">
        <MessagePanelHeader
          icon={<FileDiff className="size-3.5" style={{ color: "#5B9BD5" }} />}
          title="文件改动"
          meta={(
            <div className="flex min-w-0 items-center gap-2">
              <span data-testid="diff-file-path" className="truncate font-mono">{filePath || "未命名文件"}</span>
              <span data-testid="diff-stat" className="shrink-0 font-mono">
                <span className="forge-diff-stat-add">+{parsed.additions}</span>
                <span className="mx-1">/</span>
                <span className="forge-diff-stat-remove">-{parsed.deletions}</span>
              </span>
            </div>
          )}
          actions={(
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
          )}
        />
        <div data-testid="diff-summary" className="forge-diff-summary">
          <span>{parsed.hunkCount} 个变更块</span>
          {firstChangedLine ? <span>首处第 {firstChangedLine} 行</span> : null}
          <span>{parsed.lines.length} 行</span>
        </div>
        <div className="forge-diff-body">
          {visibleLines.map((line, i) => (
            <div
              key={`${i}-${line.raw}`}
              data-testid={`diff-line-${line.type === "add" ? "added" : line.type === "remove" ? "removed" : line.type}`}
              className={DIFF_LINE_CLASS[line.type]}
            >
              <span data-testid="diff-line-old-number" className="forge-diff-line-number">
                {line.oldNumber ?? ""}
              </span>
              <span data-testid="diff-line-new-number" className="forge-diff-line-number">
                {line.newNumber ?? ""}
              </span>
              <span className="forge-diff-line-code">{line.raw || " "}</span>
            </div>
          ))}
        </div>
        {isLongDiff && (
          <div className="forge-diff-footer">
            <button
              type="button"
              onClick={() => setExpanded((current) => !current)}
              className="forge-diff-expand"
            >
              <ChevronDown className={`size-3 transition-transform ${expanded ? "rotate-180" : ""}`} />
              {expanded ? "收起改动" : "展开完整改动"}
              {!expanded && (
                <span className="font-mono text-[10px] text-muted-foreground/70">+{hiddenLineCount} 行</span>
              )}
            </button>
          </div>
        )}
        <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
      </MessagePanel>
    </div>
  );
}

function parseDiff(diff: string) {
  let oldLine = 0;
  let newLine = 0;
  let additions = 0;
  let deletions = 0;
  let hunkCount = 0;
  let firstChangedLine: number | null = null;

  const lines = diff.split("\n").map<ParsedDiffLine>((raw) => {
    const hunk = raw.match(/^@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)(?:,\d+)?/);
    if (hunk) {
      oldLine = Number.parseInt(hunk[1], 10);
      newLine = Number.parseInt(hunk[2], 10);
      hunkCount += 1;
      return { raw, type: "hunk", oldNumber: null, newNumber: null };
    }

    if (raw.startsWith("+") && !raw.startsWith("+++")) {
      const lineNumber = newLine || null;
      additions += 1;
      if (!firstChangedLine && lineNumber) firstChangedLine = lineNumber;
      if (newLine) newLine += 1;
      return { raw, type: "add", oldNumber: null, newNumber: lineNumber };
    }

    if (raw.startsWith("-") && !raw.startsWith("---")) {
      const lineNumber = oldLine || null;
      deletions += 1;
      if (!firstChangedLine) firstChangedLine = newLine || lineNumber;
      if (oldLine) oldLine += 1;
      return { raw, type: "remove", oldNumber: lineNumber, newNumber: null };
    }

    if (raw.startsWith(" ") || (oldLine > 0 && newLine > 0 && raw.trim())) {
      const currentOld = oldLine || null;
      const currentNew = newLine || null;
      if (oldLine) oldLine += 1;
      if (newLine) newLine += 1;
      return { raw, type: "context", oldNumber: currentOld, newNumber: currentNew };
    }

    return { raw, type: "header", oldNumber: null, newNumber: null };
  });

  return { lines, additions, deletions, hunkCount, firstChangedLine };
}
