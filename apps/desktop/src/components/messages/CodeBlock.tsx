import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";
import { getHighlighter, highlightCode } from "@/lib/shiki";
import { useStore } from "@/store";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  code: string;
  lang: string;
}

export function CodeBlock({ code, lang }: CodeBlockProps) {
  const theme = useStore((s) => s.theme);
  const [html, setHtml] = useState("");
  const [copied, setCopied] = useState(false);
  const label = formatLanguageLabel(lang);
  const lineCount = code ? code.split("\n").length : 0;

  useEffect(() => {
    let cancelled = false;
    (async () => {
      await getHighlighter();
      if (cancelled) return;
      const result = highlightCode(code, lang, theme);
      setHtml(result);
    })();
    return () => {
      cancelled = true;
    };
  }, [code, lang, theme]);

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
    <figure className="code-surface group my-3 overflow-hidden rounded-md border border-border bg-background shadow-none">
      <figcaption className="flex min-h-9 items-center justify-between gap-3 border-b border-border bg-muted px-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-[#4A9E6B]" />
          <span className="truncate font-mono text-[11px] font-medium uppercase tracking-normal text-muted-foreground">
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
        {html ? (
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
