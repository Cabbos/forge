import { createHighlighter, type Highlighter } from "shiki";

let hl: Highlighter | null = null;

const LANGS = [
  "rust",
  "typescript",
  "javascript",
  "tsx",
  "jsx",
  "python",
  "bash",
  "shell",
  "json",
  "toml",
  "yaml",
  "html",
  "css",
  "sql",
  "markdown",
];

export async function getHighlighter(): Promise<Highlighter> {
  if (!hl) {
    hl = await createHighlighter({
      themes: ["github-dark", "github-light"],
      langs: LANGS,
    });
  }
  return hl;
}

export function highlightCode(
  code: string,
  lang: string,
  theme: "light" | "dark"
): string {
  if (!hl) return escapeHtml(code);

  const resolvedTheme =
    theme === "dark" ? "github-dark" : "github-light";
  const resolvedLang = LANGS.includes(lang) ? lang : "text";

  try {
    return hl.codeToHtml(code, {
      lang: resolvedLang,
      theme: resolvedTheme,
    });
  } catch {
    return escapeHtml(code);
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
