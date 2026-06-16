export interface WritePreviewView {
  filePath: string;
  mode: "markdown" | "code" | "text" | "image";
  language: string;
  languageLabel: string;
  content: string;
  lineCount: number;
  imageSrc?: string;
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
  gif: "gif",
  jpeg: "jpeg",
  jpg: "jpg",
  png: "png",
  rs: "rust",
  svg: "svg",
  ts: "typescript",
  tsx: "tsx",
  txt: "text",
  webp: "webp",
};

const IMAGE_LANGUAGES = new Set(["gif", "jpeg", "jpg", "png", "svg", "webp"]);

export function deriveWritePreview(toolName: string, input: unknown): WritePreviewView | null {
  if (!WRITE_TOOL_NAMES.has(toolName)) return null;
  if (!input || typeof input !== "object" || Array.isArray(input)) return null;

  const data = input as Record<string, unknown>;
  const filePath = firstString(data, "path", "file_path", "filename", "target_path");
  const content = firstString(data, "content", "new_content", "new_string", "replacement", "text");
  if (!filePath || !content) return null;

  const language = inferLanguage(filePath);
  const imageSrc = buildImagePreviewSrc(language, content);
  return {
    filePath,
    mode: previewMode(language, imageSrc),
    language,
    languageLabel: formatLanguageLabel(language),
    content,
    lineCount: countLines(content),
    imageSrc,
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

function previewMode(language: string, imageSrc?: string): WritePreviewView["mode"] {
  if (IMAGE_LANGUAGES.has(language)) return imageSrc ? "image" : "text";
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

function buildImagePreviewSrc(language: string, content: string) {
  if (!IMAGE_LANGUAGES.has(language)) return undefined;

  const trimmed = content.trim();
  if (/^data:image\/(?:gif|jpe?g|png|svg\+xml|webp);/i.test(trimmed)) {
    return trimmed;
  }

  if (language === "svg") {
    const svg = trimmed.replace(/^\uFEFF/, "");
    if (/^(?:<\?xml[^>]*>\s*)?<svg[\s>]/i.test(svg)) {
      return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
    }
  }

  const mimeType = imageMimeType(language);
  const base64 = trimmed.replace(/\s+/g, "");
  if (mimeType && /^[A-Za-z0-9+/]+={0,2}$/.test(base64) && base64.length >= 8) {
    return `data:${mimeType};base64,${base64}`;
  }

  return undefined;
}

function imageMimeType(language: string) {
  if (language === "jpg" || language === "jpeg") return "image/jpeg";
  if (language === "png") return "image/png";
  if (language === "gif") return "image/gif";
  if (language === "webp") return "image/webp";
  return undefined;
}
