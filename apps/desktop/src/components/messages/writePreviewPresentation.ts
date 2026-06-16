export interface WritePreviewView {
  filePath: string;
  mode: "markdown" | "code" | "text";
  language: string;
  languageLabel: string;
  content: string;
  lineCount: number;
}

const WRITE_TOOL_NAMES = new Set(["write_file", "write_to_file", "write", "edit", "replace"]);

const EXTENSION_LANGUAGE: Record<string, string> = {
  css: "css",
  html: "html",
  js: "javascript",
  jsx: "jsx",
  json: "json",
  md: "markdown",
  mdx: "markdown",
  mjs: "javascript",
  rs: "rust",
  ts: "typescript",
  tsx: "tsx",
  txt: "text",
};

export function deriveWritePreview(toolName: string, input: unknown): WritePreviewView | null {
  if (!WRITE_TOOL_NAMES.has(toolName)) return null;
  if (!input || typeof input !== "object" || Array.isArray(input)) return null;

  const data = input as Record<string, unknown>;
  const filePath = firstString(data, "path", "file_path", "filename", "target_path");
  const content = firstString(data, "content", "new_content", "new_string", "replacement", "text");
  if (!filePath || !content) return null;

  const language = inferLanguage(filePath);
  return {
    filePath,
    mode: previewMode(language),
    language,
    languageLabel: formatLanguageLabel(language),
    content,
    lineCount: countLines(content),
  };
}

function firstString(data: Record<string, unknown>, ...keys: string[]) {
  for (const key of keys) {
    const value = data[key];
    if (typeof value === "string" && value.length > 0) return value;
  }
  return "";
}

function inferLanguage(filePath: string) {
  const name = filePath.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  if (name === "readme" || name === "changelog") return "markdown";
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  return EXTENSION_LANGUAGE[extension] ?? "text";
}

function previewMode(language: string): WritePreviewView["mode"] {
  if (language === "markdown") return "markdown";
  if (language === "text") return "text";
  return "code";
}

function countLines(content: string) {
  if (!content) return 0;
  return content.replace(/\n$/, "").split("\n").length;
}

function formatLanguageLabel(language: string) {
  if (language === "markdown") return "Markdown";
  if (language === "text") return "Text";
  return language.toUpperCase();
}
