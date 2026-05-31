import { useEffect, useState } from "react";
import { useStore } from "@/store";
import { deriveCodeBlockView } from "@/components/messages/codeBlockPresentation";
import { ReaderCaptionAction } from "@/components/messages/ReaderCaptionAction";

interface CodeBlockProps {
  code: string;
  lang: string;
  streaming?: boolean;
}

const highlightedCodeCache = new Map<string, string>();
const MAX_HIGHLIGHT_CACHE_ENTRIES = 200;

function setHighlightedCodeCache(key: string, html: string) {
  if (!highlightedCodeCache.has(key) && highlightedCodeCache.size >= MAX_HIGHLIGHT_CACHE_ENTRIES) {
    const oldestKey = highlightedCodeCache.keys().next().value;
    if (oldestKey) highlightedCodeCache.delete(oldestKey);
  }
  highlightedCodeCache.set(key, html);
}

export function CodeBlock({ code, lang, streaming = false }: CodeBlockProps) {
  const theme = useStore((s) => s.theme);
  const seedView = deriveCodeBlockView({ code, lang, theme, streaming, highlightedHtml: "" });
  const cachedHtml = highlightedCodeCache.get(seedView.cacheKey) ?? "";
  const [htmlState, setHtmlState] = useState<{ key: string; html: string }>({
    key: seedView.cacheKey,
    html: cachedHtml,
  });
  const html = htmlState.key === seedView.cacheKey ? htmlState.html : cachedHtml;
  const view = deriveCodeBlockView({ code, lang, theme, streaming, highlightedHtml: html });
  const { cacheKey } = view;

  useEffect(() => {
    if (streaming) return;

    const cached = highlightedCodeCache.get(cacheKey);
    if (cached) {
      setHtmlState({ key: cacheKey, html: cached });
      return;
    }

    let cancelled = false;
    (async () => {
      const { highlightCode } = await import("@/lib/shiki");
      if (cancelled) return;
      const result = await highlightCode(code, lang, theme);
      setHighlightedCodeCache(cacheKey, result);
      setHtmlState({ key: cacheKey, html: result });
    })();
    return () => {
      cancelled = true;
    };
  }, [cacheKey, code, lang, streaming, theme]);

  return (
    <figure
      className="code-surface group my-2.5 overflow-hidden rounded-md border border-border bg-background shadow-none"
      data-renderer={view.renderer}
    >
      <figcaption className="code-caption">
        <div className="flex min-w-0 items-center gap-2">
          <span className="code-caption-dot" />
          <span className="truncate font-mono text-[10px] font-medium uppercase tracking-normal text-muted-foreground">
            {view.label}
          </span>
          {view.lineCount > 1 && (
            <span className="hidden font-mono text-[10px] text-muted-foreground sm:inline">
              {view.lineCount} 行
            </span>
          )}
        </div>
        <ReaderCaptionAction text={code} idleLabel="复制代码" />
      </figcaption>
      <div className="code-scroll max-h-[520px] overflow-auto">
        {view.shouldShowHighlighted ? (
          <div className="shiki-wrapper" dangerouslySetInnerHTML={{ __html: html }} />
        ) : (
          <pre className="code-fallback">
            <code className={`language-${lang}`}>{code}</code>
          </pre>
        )}
      </div>
    </figure>
  );
}
