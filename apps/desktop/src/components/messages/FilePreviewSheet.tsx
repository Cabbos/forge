import { useEffect, useMemo, useState } from "react";
import { Copy, ExternalLink, FileText, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { openFile, previewFile, type FilePreview } from "@/lib/tauri";

export interface FileRef {
  path: string;
  line?: number;
}

interface FilePreviewSheetProps {
  fileRef: FileRef | null;
  onClose: () => void;
  sessionId?: string;
}

export function FilePreviewSheet({ fileRef, onClose, sessionId }: FilePreviewSheetProps) {
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!fileRef) return;

    let cancelled = false;
    setLoading(true);
    setError(null);
    setPreview(null);
    setCopied(false);

    previewFile(fileRef.path, fileRef.line, sessionId)
      .then((result) => {
        if (!cancelled) setPreview(result);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [fileRef, sessionId]);

  const title = preview?.display_path || fileRef?.path || "文件预览";
  const locationLabel = useMemo(() => {
    const line = preview?.requested_line ?? fileRef?.line;
    return line ? `第 ${line} 行` : "文件开头";
  }, [fileRef?.line, preview?.requested_line]);

  const copyLabel = copied ? "已复制" : "复制路径";
  const copyText = preview
    ? `${preview.display_path}${preview.requested_line ? `:${preview.requested_line}` : ""}`
    : fileRef
      ? `${fileRef.path}${fileRef.line ? `:${fileRef.line}` : ""}`
      : "";

  const copyPath = async () => {
    if (!copyText) return;
    await navigator.clipboard?.writeText(copyText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  const openExternally = () => {
    if (!fileRef) return;
    openFile(fileRef.path, fileRef.line, sessionId).catch((err) => setError(String(err)));
  };

  return (
    <Dialog open={Boolean(fileRef)} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent
        className="!h-[min(820px,calc(100vh-48px))] !w-[min(1120px,calc(100vw-48px))] !max-w-[min(1120px,calc(100vw-48px))] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0"
        showCloseButton
      >
        <DialogHeader className="border-b border-border p-4 pr-12">
          <DialogTitle className="flex items-center gap-2 min-w-0">
            <FileText className="size-4 text-primary shrink-0" />
            <span className="truncate font-mono text-sm">{title}</span>
          </DialogTitle>
          <DialogDescription>{locationLabel}</DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-auto bg-background">
          {loading && (
            <div className="flex h-full min-h-[240px] items-center justify-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" />
              正在读取文件
            </div>
          )}

          {!loading && error && (
            <div className="p-4 text-sm leading-6 text-muted-foreground">
              <div className="rounded-lg border border-border bg-muted/20 p-3">
                <p className="font-medium text-foreground">无法预览这个文件</p>
                <p className="mt-1 break-words">{error}</p>
              </div>
            </div>
          )}

          {!loading && preview && (
            <div className="font-mono text-[12px] leading-5">
              {preview.lines.map((line) => (
                <div
                  key={line.number}
                  className="grid min-w-full grid-cols-[64px_minmax(0,1fr)] border-l-2"
                  style={{
                    borderLeftColor: line.is_target ? "var(--forge-icon-context)" : "transparent",
                    background: line.is_target ? "rgba(91,155,213,0.13)" : "transparent",
                  }}
                >
                  <div
                    className="select-none px-3 py-0.5 text-right"
                    style={{ color: line.is_target ? "#8FC7FF" : "var(--muted-foreground)" }}
                  >
                    {line.number}
                  </div>
                  <pre className="m-0 whitespace-pre-wrap break-words px-3 py-0.5 text-[#c9c9c9]">
                    {line.content || " "}
                  </pre>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between border-t border-border bg-popover p-3">
          <Button variant="outline" size="sm" onClick={copyPath} disabled={!copyText}>
            <Copy className="size-3.5" />
            {copyLabel}
          </Button>
          <Button variant="secondary" size="sm" onClick={openExternally} disabled={!fileRef}>
            <ExternalLink className="size-3.5" />
            在编辑器打开
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
