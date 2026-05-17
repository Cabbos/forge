import { useState, useEffect, useRef, useCallback } from "react";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";
import { FilePreviewSheet, type FileRef } from "@/components/messages/FilePreviewSheet";
import { Loader2 } from "lucide-react";

const STREAM_THROTTLE_MS = 220;
const FILE_REF_PREFIX = "#file-ref=";

// Match file:line patterns like src/foo.rs:42 or path/to/file.ts:123
const FILE_REF_RE = /(^|[^\w/.-])((?:\.{1,2}\/|\/)?[\w@./-]+\.[A-Za-z0-9]{1,8}):(\d+)(?::(\d+))?/g;
const INLINE_FILE_REF_RE = /^((?:\.{1,2}\/|\/)?[\w@./-]+\.[A-Za-z0-9]{1,8}):(\d+)(?::\d+)?$/;

const fileRefStyle = {
  color: "#5B9BD5",
  textDecoration: "underline",
  cursor: "pointer",
} as const;

function linkifyFileRefs(text: string): string {
  return text.replace(FILE_REF_RE, (_, prefix, path, line, column) => {
    const label = `${path}:${line}${column ? `:${column}` : ""}`;
    const payload = encodeURIComponent(`${path}:${line}`);
    return `${prefix}[${label}](${FILE_REF_PREFIX}${payload})`;
  });
}

function preprocessFileRefs(text: string): string {
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

function parseFileRef(href: string): { path: string; line?: number } | null {
  if (!href.startsWith(FILE_REF_PREFIX)) return null;

  const decoded = decodeURIComponent(href.slice(FILE_REF_PREFIX.length));
  const match = decoded.match(/:(\d+)$/);
  if (!match) return { path: decoded };

  const path = decoded.slice(0, -match[0].length);
  const line = Number.parseInt(match[1], 10);
  return path ? { path, line: Number.isFinite(line) ? line : undefined } : null;
}

function parseInlineFileRef(text: string): { path: string; line?: number; label: string } | null {
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

function FileRefLink({
  path,
  line,
  children,
  onOpen,
}: {
  path: string;
  line?: number;
  children: ReactNode;
  onOpen: (fileRef: FileRef) => void;
}) {
  const href = `${FILE_REF_PREFIX}${encodeURIComponent(`${path}:${line ?? ""}`)}`;

  return (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        onOpen({ path, line });
      }}
      style={fileRefStyle}
    >
      {children}
    </a>
  );
}

function TextWithFileRefs({ content, onOpenFileRef }: { content: string; onOpenFileRef: (fileRef: FileRef) => void }) {
  const parts: ReactNode[] = [];
  const regex = new RegExp(FILE_REF_RE);
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = regex.exec(content)) !== null) {
    const [fullMatch, prefix, path, line, column] = match;
    const prefixStart = match.index;
    const linkStart = prefixStart + prefix.length;
    const fullEnd = prefixStart + fullMatch.length;

    if (linkStart > lastIndex) {
      parts.push(content.slice(lastIndex, linkStart));
    }

    const lineNumber = Number.parseInt(line, 10);
    parts.push(
      <FileRefLink
        key={key++}
        path={path}
        line={Number.isFinite(lineNumber) ? lineNumber : undefined}
        onOpen={onOpenFileRef}
      >
        {`${path}:${line}${column ? `:${column}` : ""}`}
      </FileRefLink>
    );

    lastIndex = fullEnd;
  }

  if (lastIndex < content.length) {
    parts.push(content.slice(lastIndex));
  }

  return <>{parts}</>;
}

function MarkdownRenderer({ content, onOpenFileRef }: { content: string; onOpenFileRef: (fileRef: FileRef) => void }) {
  const handleFileClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    const link = target.closest("a");
    if (!link) return;
    const href = link.getAttribute("href") || "";
    const fileRef = parseFileRef(href);
    if (fileRef) {
      e.preventDefault();
      onOpenFileRef(fileRef);
    }
  }, [onOpenFileRef]);

  const processed = preprocessFileRefs(content);

  return (
    <div onClick={handleFileClick}>
      <ReactMarkdown remarkPlugins={[remarkGfm]}
        components={{
          code({ className, children }) {
            const match = /language-(\w+)/.exec(className || "");
            if (!className) {
              const inlineFileRef = parseInlineFileRef(String(children));
              if (inlineFileRef) {
                return (
                  <code style={{ color: "#5B9BD5" }}>
                    <FileRefLink path={inlineFileRef.path} line={inlineFileRef.line} onOpen={onOpenFileRef}>
                      {inlineFileRef.label}
                    </FileRefLink>
                  </code>
                );
              }

              return <code style={{ color: "#D4A853" }}>{children}</code>;
            }
            return <CodeBlock code={String(children).replace(/\n$/, "")} lang={match?.[1] || ""} />;
          },
          pre({ children }) { return <>{children}</>; },
          a({ href, children }) {
            const fileRef = href ? parseFileRef(href) : null;
            if (fileRef) {
              return (
                <FileRefLink path={fileRef.path} line={fileRef.line} onOpen={onOpenFileRef}>
                  {children}
                </FileRefLink>
              );
            }
            return <a href={href} target="_blank" rel="noreferrer" style={{ color: "#D4A853" }}>{children}</a>;
          },
        }}>
        {processed}
      </ReactMarkdown>
    </div>
  );
}

export function TextBlock({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  if (!block.content && block.isComplete) return null;
  const hasContent = Boolean(block.content);

  // During streaming, throttle content updates to avoid ReactMarkdown flicker
  const [displayContent, setDisplayContent] = useState(block.content);
  const [previewFileRef, setPreviewFileRef] = useState<FileRef | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastUpdateRef = useRef(0);

  useEffect(() => {
    if (block.isComplete) {
      // Immediately show final content on completion
      if (timerRef.current) clearTimeout(timerRef.current);
      setDisplayContent(block.content);
      return;
    }
    // During streaming, throttle to every STREAM_THROTTLE_MS
    const now = Date.now();
    const elapsed = now - lastUpdateRef.current;
    if (elapsed >= STREAM_THROTTLE_MS) {
      lastUpdateRef.current = now;
      setDisplayContent(block.content);
    } else {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        lastUpdateRef.current = Date.now();
        setDisplayContent(block.content);
      }, STREAM_THROTTLE_MS - elapsed);
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [block.content, block.isComplete]);

  const renderedContent = block.isComplete ? block.content : displayContent;

  return (
    <div>
      {hasContent ? (
        <div
          data-testid="assistant-message"
          className="min-w-0 py-1 text-left text-sm leading-7 break-words"
          style={{ color: "var(--foreground)", overflowWrap: "anywhere" }}
        >
          <div className="markdown-content">
            {block.isComplete ? (
              <MarkdownRenderer content={renderedContent} onOpenFileRef={setPreviewFileRef} />
            ) : (
              <div className="whitespace-pre-wrap break-words" style={{ overflowWrap: "anywhere" }}>
                <TextWithFileRefs content={renderedContent} onOpenFileRef={setPreviewFileRef} />
              </div>
            )}
          </div>
        </div>
      ) : (
        <div className="py-1 text-muted-foreground/70">
          <Loader2 className="size-4 animate-spin" />
        </div>
      )}
      <FilePreviewSheet fileRef={previewFileRef} sessionId={sessionId} onClose={() => setPreviewFileRef(null)} />
    </div>
  );
}
