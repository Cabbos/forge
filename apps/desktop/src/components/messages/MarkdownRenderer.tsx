import { useCallback, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { CodeBlock } from "@/components/messages/CodeBlock";
import { DiagramBlock } from "@/components/messages/DiagramBlock";
import { shouldRenderDiagram } from "@/components/messages/diagramPresentation";
import type { FileRef } from "@/components/messages/filePreviewTypes";
import { FileRefLink, parseFileRef, parseInlineFileRef, preprocessFileRefs } from "@/components/messages/markdownFileRefs";

const LONG_REPLY_SECTION_INDEX_MIN_CHARS = 320;
const LONG_REPLY_SECTION_INDEX_MIN_HEADINGS = 3;

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
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
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
            const codeText = String(children).replace(/\n$/, "");
            if (!className) {
              if (codeText.includes("\n") && shouldRenderDiagram(codeText, "")) {
                return <DiagramBlock code={codeText} lang="" />;
              }
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
            const lang = match?.[1] || "";
            if (shouldRenderDiagram(codeText, lang)) {
              return <DiagramBlock code={codeText} lang={lang} />;
            }
            return <CodeBlock code={codeText} lang={lang} streaming={streaming} />;
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
        }}
      >
        {processed}
      </ReactMarkdown>
    </div>
  );
}
