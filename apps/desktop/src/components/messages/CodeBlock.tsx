import { useEffect, useState } from "react";
import { getHighlighter, highlightCode } from "@/lib/shiki";
import { useStore } from "@/store";

interface CodeBlockProps {
  code: string;
  lang: string;
}

export function CodeBlock({ code, lang }: CodeBlockProps) {
  const theme = useStore((s) => s.theme);
  const [html, setHtml] = useState("");

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

  if (!html) {
    // Fallback before Shiki loads
    return (
      <pre className="!bg-muted !border !border-border !rounded-lg !p-3 overflow-x-auto">
        <code className={`language-${lang}`}>{code}</code>
      </pre>
    );
  }

  return (
    <div
      className="shiki-wrapper max-w-full [&_.shiki]:!p-3 [&_.shiki]:!rounded-lg [&_.shiki]:overflow-x-auto [&_.shiki]:border [&_.shiki]:border-border [&_code]:!text-[13px] [&_code]:font-mono [&_code]:whitespace-pre-wrap [&_code]:break-all"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
