import type { CSSProperties } from "react";
import type { FilePreview, FilePreviewLine } from "@/lib/tauri";

interface FilePreviewRef {
  path: string;
  line?: number;
}

interface FilePreviewViewInput {
  fileRef: FilePreviewRef | null;
  preview: FilePreview | null;
}

export interface FilePreviewView {
  title: string;
  locationLabel: string;
  copyText: string;
  lines: FilePreviewLine[] | null;
}

export function deriveFilePreviewView({ fileRef, preview }: FilePreviewViewInput): FilePreviewView {
  const line = preview?.requested_line ?? fileRef?.line ?? null;
  const title = preview?.display_path || fileRef?.path || "文件预览";
  const locationLabel = line ? `第 ${line} 行` : "文件开头";
  const copyText = preview
    ? `${preview.display_path}${preview.requested_line ? `:${preview.requested_line}` : ""}`
    : fileRef
      ? `${fileRef.path}${fileRef.line ? `:${fileRef.line}` : ""}`
      : "";

  return {
    title,
    locationLabel,
    copyText,
    lines: preview?.lines ?? null,
  };
}

export function lineTone(line: FilePreviewLine): {
  row: CSSProperties;
  number: CSSProperties;
} {
  return {
    row: {
      borderLeftColor: line.is_target ? "var(--forge-icon-context)" : "transparent",
      background: line.is_target ? "rgba(91,155,213,0.13)" : "transparent",
    },
    number: {
      color: line.is_target ? "#8FC7FF" : "var(--muted-foreground)",
    },
  };
}
