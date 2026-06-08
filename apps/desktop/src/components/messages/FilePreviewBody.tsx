import { Loader2 } from "lucide-react";
import type { FilePreviewLine } from "@/lib/tauri";
import { lineTone } from "@/components/messages/filePreviewPresentation";

interface FilePreviewBodyProps {
  loading: boolean;
  error: string | null;
  lines: FilePreviewLine[] | null;
}

export function FilePreviewBody({ loading, error, lines }: FilePreviewBodyProps) {
  if (loading) {
    return (
      <div className="flex h-full min-h-[240px] items-center justify-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        正在读取文件
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 text-sm leading-6 text-muted-foreground">
        <div className="rounded-lg border border-border bg-muted/20 p-3">
          <p className="font-medium text-foreground">无法预览这个文件</p>
          <p className="mt-1 break-words">{error}</p>
        </div>
      </div>
    );
  }

  if (!lines) return null;

  return (
    <div className="font-mono text-[12px] leading-5">
      {lines.map((line) => {
        const tone = lineTone(line);
        return (
          <div
            key={line.number}
            className="grid min-w-full grid-cols-[64px_minmax(0,1fr)] border-b border-[var(--forge-border-subtle)] last:border-b-0"
            style={tone.row}
          >
            <div className="select-none px-3 py-0.5 text-right" style={tone.number}>
              {line.number}
            </div>
            <pre className="m-0 whitespace-pre-wrap break-words px-3 py-0.5 text-[var(--forge-code-text)]">
              {line.content || " "}
            </pre>
          </div>
        );
      })}
    </div>
  );
}
