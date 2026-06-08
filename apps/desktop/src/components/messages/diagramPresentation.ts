const DIAGRAM_LANGS = new Set(["diagram", "ascii", "text", "txt", "plain", "plaintext"]);
const MERMAID_LANGS = new Set(["mermaid", "mmd"]);

export function deriveDiagramView(code: string, lang: string) {
  const kind = isMermaidLanguage(lang) ? "mermaid" : "ascii";
  return {
    kind,
    title: kind === "mermaid" ? "Mermaid 图" : "架构图",
    meta: kind === "mermaid" ? "可复制源码" : `${code.split("\n").length} 行`,
  };
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
