import { useState } from "react";
import { Check, Copy, Network } from "lucide-react";
import { cn } from "@/lib/utils";

interface DiagramBlockProps {
  code: string;
  lang: string;
}

const DIAGRAM_LANGS = new Set(["diagram", "ascii", "text", "txt", "plain", "plaintext"]);
const MERMAID_LANGS = new Set(["mermaid", "mmd"]);

export function DiagramBlock({ code, lang }: DiagramBlockProps) {
  const [copied, setCopied] = useState(false);
  const kind = isMermaidLanguage(lang) ? "mermaid" : "ascii";
  const title = kind === "mermaid" ? "Mermaid 图" : "架构图";
  const meta = kind === "mermaid" ? "可复制源码" : `${code.split("\n").length} 行`;

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
    <figure data-testid="diagram-surface" data-diagram-kind={kind} className="diagram-surface">
      <figcaption className="diagram-caption">
        <div className="diagram-caption-title">
          <Network className="size-3.5" />
          <span>{title}</span>
          <span className="diagram-caption-meta">{meta}</span>
        </div>
        <button
          type="button"
          onClick={copy}
          className={cn(
            "inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors",
            "hover:bg-secondary hover:text-foreground focus:outline-none focus:ring-1 focus:ring-primary/60"
          )}
          aria-label={copied ? "已复制" : "复制图示源码"}
          title={copied ? "已复制" : "复制图示源码"}
        >
          {copied ? <Check className="size-3.5 text-[#4A9E6B]" /> : <Copy className="size-3.5" />}
        </button>
      </figcaption>
      <div data-testid="diagram-viewport" className="diagram-viewport">
        <pre className="diagram-code">
          <code>{code}</code>
        </pre>
      </div>
    </figure>
  );
}

export function shouldRenderDiagram(code: string, lang: string) {
  if (isMermaidLanguage(lang)) return true;
  const normalizedLang = lang.trim().toLowerCase();
  if (DIAGRAM_LANGS.has(normalizedLang) && looksLikeAsciiDiagram(code)) return true;
  if (!normalizedLang && looksLikeAsciiDiagram(code)) return true;
  return false;
}

function isMermaidLanguage(lang: string) {
  return MERMAID_LANGS.has(lang.trim().toLowerCase());
}

function looksLikeAsciiDiagram(code: string) {
  const lines = code.split("\n").filter((line) => line.trim().length > 0);
  if (lines.length < 3) return false;

  const diagramGlyphs = code.match(/[┌┐└┘├┤┬┴┼│─╭╮╰╯╠╣╦╩╬═║+|<>→←↑↓↔▼▲]/g)?.length ?? 0;
  const connectorRuns = code.match(/(?:-{2,}|={2,}|>{1,}|<-|->|\|)/g)?.length ?? 0;
  const boxLikeLines = lines.filter((line) => /[┌┐└┘├┤┬┴┼│─+|]/.test(line) && line.length >= 8).length;
  const arrowLines = lines.filter((line) => /(?:->|<-|→|←|↑|↓|▼|▲)/.test(line)).length;
  const density = diagramGlyphs / Math.max(code.length, 1);

  return boxLikeLines >= 2 && (arrowLines >= 1 || connectorRuns >= 4 || density > 0.08);
}
