export function deriveCodeBlockView({
  code,
  lang,
  theme,
  streaming,
  highlightedHtml,
}: {
  code: string;
  lang: string;
  theme: string;
  streaming: boolean;
  highlightedHtml: string;
}) {
  const cacheKey = `${theme}:${lang}:${code}`;
  const label = formatLanguageLabel(lang);
  const lineCount = code ? code.split("\n").length : 0;
  const shouldShowHighlighted = !streaming && Boolean(highlightedHtml);
  const renderer = shouldShowHighlighted ? "highlighted" : "plain";

  return {
    cacheKey,
    label,
    lineCount,
    shouldShowHighlighted,
    renderer,
  };
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
