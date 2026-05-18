import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";
import { getHighlighter, highlightCode } from "@/lib/shiki";
import { useStore } from "@/store";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  code: string;
  lang: string;
  streaming?: boolean;
}

const highlightedCodeCache = new Map<string, string>();

export function CodeBlock({ code, lang, streaming = false }: CodeBlockProps) {
  const theme = useStore((s) => s.theme);
  const cacheKey = `${theme}:${lang}:${code}`;
  const cachedHtml = highlightedCodeCache.get(cacheKey) ?? "";
  const [htmlState, setHtmlState] = useState<{ key: string; html: string }>({
    key: cacheKey,
    html: cachedHtml,
  });
  const [copied, setCopied] = useState(false);
  const label = formatLanguageLabel(lang);
  const lineCount = code ? code.split("\n").length : 0;
  const html = htmlState.key === cacheKey ? htmlState.html : cachedHtml;
  const shouldShowHighlighted = !streaming && Boolean(html);
  const renderer = shouldShowHighlighted ? "highlighted" : "plain";

  useEffect(() => {
    if (streaming) return;

    const cached = highlightedCodeCache.get(cacheKey);
    if (cached) {
      setHtmlState({ key: cacheKey, html: cached });
      return;
    }

    let cancelled = false;
    (async () => {
      await getHighlighter();
      if (cancelled) return;
      const result = highlightCode(code, lang, theme);
      highlightedCodeCache.set(cacheKey, result);
      setHtmlState({ key: cacheKey, html: result });
    })();
    return () => {
      cancelled = true;
    };
  }, [cacheKey, code, lang, streaming, theme]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      setCopied(false);
    }
  };

  return (
    <figure
      className="code-surface group my-2.5 overflow-hidden rounded-md border border-border bg-background shadow-none"
      data-renderer={renderer}
    >
      <figcaption className="code-caption">
        <div className="flex min-w-0 items-center gap-2">
          <span className="code-caption-dot" />
          <span className="truncate font-mono text-[10px] font-medium uppercase tracking-normal text-muted-foreground">
            {label}
          </span>
          {lineCount > 1 && (
            <span className="hidden font-mono text-[10px] text-muted-foreground sm:inline">
              {lineCount} 行
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={copy}
          className={cn(
            "inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors",
            "hover:bg-secondary hover:text-foreground focus:outline-none focus:ring-1 focus:ring-primary/60"
          )}
          aria-label={copied ? "已复制" : "复制代码"}
          title={copied ? "已复制" : "复制代码"}
        >
          {copied ? <Check className="size-3.5 text-[#4A9E6B]" /> : <Copy className="size-3.5" />}
        </button>
      </figcaption>
      <div className="code-scroll max-h-[520px] overflow-auto">
        {shouldShowHighlighted ? (
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

function formatLanguageLabel(lang: string): string {
  const value = lang.trim().toLowerCase();
  if (!value) return "文本";
  const labels: Record<string, string> = {
    js: "javascript",
    jsx: "jsx",
    ts: "typescript",
    tsx: "tsx",
    rs: "rust",
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    md: "markdown",
    yml: "yaml",
  };
  return labels[value] || value;
}
