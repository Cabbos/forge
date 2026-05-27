import { createHighlighterCore, type HighlighterCore, type LanguageInput } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import githubDark from "shiki/dist/themes/github-dark.mjs";
import githubLight from "shiki/dist/themes/github-light.mjs";

let hl: HighlighterCore | null = null;

const LANG_LOADERS = {
  rust: () => import("shiki/dist/langs/rust.mjs").then((module) => module.default),
  typescript: () => import("shiki/dist/langs/typescript.mjs").then((module) => module.default),
  javascript: () => import("shiki/dist/langs/javascript.mjs").then((module) => module.default),
  tsx: () => import("shiki/dist/langs/tsx.mjs").then((module) => module.default),
  jsx: () => import("shiki/dist/langs/jsx.mjs").then((module) => module.default),
  python: () => import("shiki/dist/langs/python.mjs").then((module) => module.default),
  bash: () => import("shiki/dist/langs/bash.mjs").then((module) => module.default),
  shell: () => import("shiki/dist/langs/shell.mjs").then((module) => module.default),
  json: () => import("shiki/dist/langs/json.mjs").then((module) => module.default),
  toml: () => import("shiki/dist/langs/toml.mjs").then((module) => module.default),
  yaml: () => import("shiki/dist/langs/yaml.mjs").then((module) => module.default),
  html: () => import("shiki/dist/langs/html.mjs").then((module) => module.default),
  css: () => import("shiki/dist/langs/css.mjs").then((module) => module.default),
  sql: () => import("shiki/dist/langs/sql.mjs").then((module) => module.default),
  markdown: () => import("shiki/dist/langs/markdown.mjs").then((module) => module.default),
} satisfies Record<string, () => Promise<LanguageInput>>;

type SupportedLang = keyof typeof LANG_LOADERS;

const LANG_ALIASES: Record<string, SupportedLang> = {
  ts: "typescript",
  js: "javascript",
  py: "python",
  sh: "bash",
  zsh: "bash",
  shellscript: "bash",
  yml: "yaml",
  md: "markdown",
};

const loadedLangs = new Set<SupportedLang>();

export async function getHighlighter(): Promise<HighlighterCore> {
  if (!hl) {
    hl = await createHighlighterCore({
      themes: [githubDark, githubLight],
      langs: [],
      engine: createJavaScriptRegexEngine(),
    });
  }
  return hl;
}

export async function highlightCode(
  code: string,
  lang: string,
  theme: "light" | "dark"
): Promise<string> {
  const highlighter = await getHighlighter();

  const resolvedTheme =
    theme === "dark" ? "github-dark" : "github-light";
  const resolvedLang = await ensureLanguageLoaded(highlighter, lang);

  try {
    return highlighter.codeToHtml(code, {
      lang: resolvedLang,
      theme: resolvedTheme,
    });
  } catch {
    return escapeHtml(code);
  }
}

async function ensureLanguageLoaded(highlighter: HighlighterCore, lang: string): Promise<SupportedLang | "text"> {
  const normalizedLang = lang.toLowerCase();
  const resolvedLang = resolveLang(normalizedLang);
  if (!resolvedLang) return "text";
  if (loadedLangs.has(resolvedLang)) return resolvedLang;

  const language = await LANG_LOADERS[resolvedLang]();
  await highlighter.loadLanguage(language);
  loadedLangs.add(resolvedLang);
  return resolvedLang;
}

function resolveLang(lang: string): SupportedLang | null {
  if (isSupportedLang(lang)) return lang;
  return LANG_ALIASES[lang] ?? null;
}

function isSupportedLang(lang: string): lang is SupportedLang {
  return Object.prototype.hasOwnProperty.call(LANG_LOADERS, lang);
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
