export const INITIAL_VISIBLE_DIFF_LINES = 28;

export type DiffLineType = "header" | "hunk" | "add" | "remove" | "context";
const INITIAL_VISIBLE_DIFF_FILES = 6;

export interface ParsedDiffLine {
  raw: string;
  type: DiffLineType;
  oldNumber: number | null;
  newNumber: number | null;
}

export interface DiffFileEntry {
  path: string;
  additions: number;
  deletions: number;
  status: "added" | "deleted" | "modified";
}

export const DIFF_LINE_CLASS: Record<DiffLineType, string> = {
  add: "forge-diff-line forge-diff-line-added",
  remove: "forge-diff-line forge-diff-line-removed",
  hunk: "forge-diff-line forge-diff-line-hunk",
  header: "forge-diff-line forge-diff-line-header",
  context: "forge-diff-line forge-diff-line-context",
};

export function deriveDiffView(diff: string, expanded: boolean) {
  const parsed = parseDiff(diff);
  const isLongDiff = parsed.lines.length > INITIAL_VISIBLE_DIFF_LINES;
  const visibleLines = expanded ? parsed.lines : parsed.lines.slice(0, INITIAL_VISIBLE_DIFF_LINES);
  const visibleFiles = parsed.files.slice(0, INITIAL_VISIBLE_DIFF_FILES);

  return {
    ...parsed,
    isLongDiff,
    visibleLines,
    hiddenLineCount: Math.max(0, parsed.lines.length - visibleLines.length),
    firstChangedLine: parsed.firstChangedLine ?? undefined,
    fileCount: parsed.files.length,
    visibleFiles,
    hiddenFileCount: Math.max(0, parsed.files.length - visibleFiles.length),
  };
}

function parseDiff(diff: string) {
  let oldLine = 0;
  let newLine = 0;
  let additions = 0;
  let deletions = 0;
  let hunkCount = 0;
  let firstChangedLine: number | null = null;

  const lines: ParsedDiffLine[] = [];
  const files: DiffFileEntry[] = [];
  let currentFile: DiffFileEntry | null = null;
  for (const raw of diff.split("\n")) {
    const fileHeader = raw.match(/^diff --git a\/(.+?) b\/(.+)$/);
    if (fileHeader) {
      const path = fileHeader[2] || fileHeader[1];
      currentFile = { path, additions: 0, deletions: 0, status: "modified" };
      files.push(currentFile);
      lines.push({ raw, type: "header", oldNumber: null, newNumber: null });
      continue;
    }

    const newFileHeader = raw.match(/^\+\+\+\s+b\/(.+)$/);
    if (newFileHeader && currentFile) {
      currentFile.path = newFileHeader[1];
    }

    if (raw === "--- /dev/null" && currentFile) {
      currentFile.status = "added";
    }

    if (raw === "+++ /dev/null" && currentFile) {
      currentFile.status = "deleted";
    }

    const hunk = raw.match(/^@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)(?:,\d+)?/);
    if (hunk) {
      oldLine = Number.parseInt(hunk[1], 10);
      newLine = Number.parseInt(hunk[2], 10);
      hunkCount += 1;
      lines.push({ raw, type: "hunk", oldNumber: null, newNumber: null });
      continue;
    }

    if (raw.startsWith("+") && !raw.startsWith("+++")) {
      const lineNumber = newLine || null;
      additions += 1;
      if (currentFile) currentFile.additions += 1;
      if (!firstChangedLine && lineNumber) firstChangedLine = lineNumber;
      if (newLine) newLine += 1;
      lines.push({ raw, type: "add", oldNumber: null, newNumber: lineNumber });
      continue;
    }

    if (raw.startsWith("-") && !raw.startsWith("---")) {
      const lineNumber = oldLine || null;
      deletions += 1;
      if (currentFile) currentFile.deletions += 1;
      if (!firstChangedLine) firstChangedLine = newLine || lineNumber;
      if (oldLine) oldLine += 1;
      lines.push({ raw, type: "remove", oldNumber: lineNumber, newNumber: null });
      continue;
    }

    if (raw.startsWith(" ") || (oldLine > 0 && newLine > 0 && raw.trim())) {
      const currentOld = oldLine || null;
      const currentNew = newLine || null;
      if (oldLine) oldLine += 1;
      if (newLine) newLine += 1;
      lines.push({ raw, type: "context", oldNumber: currentOld, newNumber: currentNew });
      continue;
    }

    lines.push({ raw, type: "header", oldNumber: null, newNumber: null });
  }

  return { lines, additions, deletions, hunkCount, firstChangedLine, files };
}
