export const INITIAL_VISIBLE_DIFF_LINES = 28;

export type DiffLineType = "header" | "hunk" | "add" | "remove" | "context";

export interface ParsedDiffLine {
  raw: string;
  type: DiffLineType;
  oldNumber: number | null;
  newNumber: number | null;
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

  return {
    ...parsed,
    isLongDiff,
    visibleLines,
    hiddenLineCount: Math.max(0, parsed.lines.length - visibleLines.length),
    firstChangedLine: parsed.firstChangedLine ?? undefined,
  };
}

function parseDiff(diff: string) {
  let oldLine = 0;
  let newLine = 0;
  let additions = 0;
  let deletions = 0;
  let hunkCount = 0;
  let firstChangedLine: number | null = null;

  const lines = diff.split("\n").map<ParsedDiffLine>((raw) => {
    const hunk = raw.match(/^@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)(?:,\d+)?/);
    if (hunk) {
      oldLine = Number.parseInt(hunk[1], 10);
      newLine = Number.parseInt(hunk[2], 10);
      hunkCount += 1;
      return { raw, type: "hunk", oldNumber: null, newNumber: null };
    }

    if (raw.startsWith("+") && !raw.startsWith("+++")) {
      const lineNumber = newLine || null;
      additions += 1;
      if (!firstChangedLine && lineNumber) firstChangedLine = lineNumber;
      if (newLine) newLine += 1;
      return { raw, type: "add", oldNumber: null, newNumber: lineNumber };
    }

    if (raw.startsWith("-") && !raw.startsWith("---")) {
      const lineNumber = oldLine || null;
      deletions += 1;
      if (!firstChangedLine) firstChangedLine = newLine || lineNumber;
      if (oldLine) oldLine += 1;
      return { raw, type: "remove", oldNumber: lineNumber, newNumber: null };
    }

    if (raw.startsWith(" ") || (oldLine > 0 && newLine > 0 && raw.trim())) {
      const currentOld = oldLine || null;
      const currentNew = newLine || null;
      if (oldLine) oldLine += 1;
      if (newLine) newLine += 1;
      return { raw, type: "context", oldNumber: currentOld, newNumber: currentNew };
    }

    return { raw, type: "header", oldNumber: null, newNumber: null };
  });

  return { lines, additions, deletions, hunkCount, firstChangedLine };
}
