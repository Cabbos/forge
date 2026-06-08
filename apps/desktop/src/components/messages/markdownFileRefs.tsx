import type { ReactNode } from "react";
import { FileText } from "lucide-react";
import type { FileRef } from "@/components/messages/filePreviewTypes";

const FILE_REF_PREFIX = "#file-ref=";

// Match file:line patterns like src/foo.rs:42 or path/to/file.ts:123
const FILE_REF_RE = /(^|[^\w/.-])((?:\.{1,2}\/|\/)?[\w@./-]+\.[A-Za-z0-9]{1,8}):(\d+)(?::(\d+))?/g;
const INLINE_FILE_REF_RE = /^((?:\.{1,2}\/|\/)?[\w@./-]+\.[A-Za-z0-9]{1,8}):(\d+)(?::\d+)?$/;

function linkifyFileRefs(text: string): string {
  return text.replace(FILE_REF_RE, (_, prefix, path, line, column) => {
    const label = `${path}:${line}${column ? `:${column}` : ""}`;
    const payload = encodeURIComponent(`${path}:${line}`);
    return `${prefix}[${label}](${FILE_REF_PREFIX}${payload})`;
  });
}

export function preprocessFileRefs(text: string): string {
  let inFence = false;

  return text
    .split("\n")
    .map((line) => {
      if (/^\s*(```|~~~)/.test(line)) {
        inFence = !inFence;
        return line;
      }
      if (inFence) return line;

      return line
        .split(/(`+[^`]*`+)/g)
        .map((part) => (part.startsWith("`") ? part : linkifyFileRefs(part)))
        .join("");
    })
    .join("\n");
}

export function parseFileRef(href: string): { path: string; line?: number } | null {
  if (!href.startsWith(FILE_REF_PREFIX)) return null;

  const decoded = decodeURIComponent(href.slice(FILE_REF_PREFIX.length));
  const match = decoded.match(/:(\d+)$/);
  if (!match) return { path: decoded };

  const path = decoded.slice(0, -match[0].length);
  const line = Number.parseInt(match[1], 10);
  return path ? { path, line: Number.isFinite(line) ? line : undefined } : null;
}

export function parseInlineFileRef(text: string): { path: string; line?: number; label: string } | null {
  const trimmed = text.trim();
  const match = trimmed.match(INLINE_FILE_REF_RE);
  if (!match) return null;

  const line = Number.parseInt(match[2], 10);
  return {
    path: match[1],
    line: Number.isFinite(line) ? line : undefined,
    label: trimmed,
  };
}

function getFileName(path: string) {
  const normalized = path.replace(/\\/g, "/");
  return normalized.split("/").filter(Boolean).pop() || path;
}

export function FileRefLink({
  path,
  line,
  onOpen,
}: {
  path: string;
  line?: number;
  children?: ReactNode;
  onOpen: (fileRef: FileRef) => void;
}) {
  const href = `${FILE_REF_PREFIX}${encodeURIComponent(`${path}:${line ?? ""}`)}`;
  const label = line ? `${path}:${line}` : path;

  return (
    <a
      href={href}
      className="forge-file-ref"
      title={label}
      aria-label={`打开 ${label}`}
      onClick={(e) => {
        e.preventDefault();
        onOpen({ path, line });
      }}
    >
      <FileText className="forge-file-ref-icon" aria-hidden="true" />
      <span className="forge-file-ref-name">{getFileName(path)}</span>
      {line && <span className="forge-file-ref-line">line {line}</span>}
    </a>
  );
}
