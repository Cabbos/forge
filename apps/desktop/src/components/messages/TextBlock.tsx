import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";
import { FilePreviewSheet, type FileRef } from "@/components/messages/FilePreviewSheet";
import { MessageCopyAction } from "@/components/messages/MessageCopyAction";
import { Loader2 } from "lucide-react";

const STREAM_THROTTLE_MS = 96;
const FILE_REF_PREFIX = "#file-ref=";
const LONG_REPLY_SECTION_INDEX_MIN_CHARS = 320;
const LONG_REPLY_SECTION_INDEX_MIN_HEADINGS = 3;

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

function stabilizeStreamingMarkdown(content: string): string {
  const lines = content.split("\n");
  let openFence: "```" | "~~~" | null = null;

  for (const line of lines) {
    const match = line.match(/^\s*(```|~~~)/);
    if (!match) continue;
    const marker = match[1] as "```" | "~~~";
    openFence = openFence === marker ? null : marker;
  }

  if (!openFence) return content;
  return `${content}\n${openFence}`;
}

interface MarkdownHeading {
  id: string;
  level: 2 | 3;
  title: string;
}

function extractMarkdownHeadings(content: string): MarkdownHeading[] {
  const headings: MarkdownHeading[] = [];
  const seen = new Map<string, number>();

  content.split("\n").forEach((line) => {
    const match = line.match(/^(#{2,3})\s+(.+?)\s*#*\s*$/);
    if (!match) return;
    const title = match[2].replace(/\[[^\]]+\]\([^)]+\)/g, "").replace(/[`*_~]/g, "").trim();
    if (!title) return;
    const base = stableHeadingId(title);
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    headings.push({
      id: count ? `${base}-${count + 1}` : base,
      level: match[1].length as 2 | 3,
      title,
    });
  });

  return headings;
}

function stableHeadingId(title: string) {
  let hash = 0;
  for (let i = 0; i < title.length; i += 1) {
    hash = (Math.imul(31, hash) + title.charCodeAt(i)) | 0;
  }
  return `forge-section-${Math.abs(hash).toString(36)}`;
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
      className="forge-file-ref"
      onClick={(e) => {
        e.preventDefault();
        onOpen({ path, line });
      }}
    >
      {children}
    </a>
  );
}

export function MarkdownRenderer({
  content,
  onOpenFileRef,
  streaming = false,
  showSectionIndex = false,
}: {
  content: string;
  onOpenFileRef: (fileRef: FileRef) => void;
  streaming?: boolean;
  showSectionIndex?: boolean;
}) {
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

  const stableContent = streaming ? stabilizeStreamingMarkdown(content) : content;
  const processed = preprocessFileRefs(stableContent);
  const headings = useMemo(() => extractMarkdownHeadings(stableContent), [stableContent]);
  const shouldShowSectionIndex = showSectionIndex &&
    !streaming &&
    stableContent.length >= LONG_REPLY_SECTION_INDEX_MIN_CHARS &&
    headings.length >= LONG_REPLY_SECTION_INDEX_MIN_HEADINGS;
  let headingRenderIndex = 0;

  return (
    <div onClick={handleFileClick}>
      {shouldShowSectionIndex && (
        <nav data-testid="answer-section-index" className="forge-answer-section-index" aria-label="回复结构">
          <span>回复结构</span>
          <div>
            {headings.slice(0, 4).map((heading) => (
              <a key={heading.id} href={`#${heading.id}`}>
                {heading.title}
              </a>
            ))}
          </div>
        </nav>
      )}
      <ReactMarkdown remarkPlugins={[remarkGfm]}
        components={{
          h2({ children }) {
            const heading = headings[headingRenderIndex];
            headingRenderIndex += 1;
            return <h2 id={heading?.id}>{children}</h2>;
          },
          h3({ children }) {
            const heading = headings[headingRenderIndex];
            headingRenderIndex += 1;
            return <h3 id={heading?.id}>{children}</h3>;
          },
          code({ className, children }) {
            const match = /language-(\w+)/.exec(className || "");
            if (!className) {
              const inlineFileRef = parseInlineFileRef(String(children));
              if (inlineFileRef) {
                return (
                  <code className="forge-inline-code forge-inline-code-file">
                    <FileRefLink path={inlineFileRef.path} line={inlineFileRef.line} onOpen={onOpenFileRef}>
                      {inlineFileRef.label}
                    </FileRefLink>
                  </code>
                );
              }

              return <code className="forge-inline-code">{children}</code>;
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
            return <a href={href} target="_blank" rel="noreferrer">{children}</a>;
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
          className="forge-message-with-actions forge-assistant-message break-words"
        >
          <MessageCopyAction text={block.content} label="回复" />
          <div className="markdown-content">
            <MarkdownRenderer
              content={renderedContent}
              onOpenFileRef={setPreviewFileRef}
              streaming={!block.isComplete}
              showSectionIndex
            />
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
