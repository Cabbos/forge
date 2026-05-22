import { ChevronDown } from "lucide-react";
import { DIFF_LINE_CLASS, type ParsedDiffLine } from "@/components/messages/diffPresentation";

interface DiffBodyProps {
  visibleLines: ParsedDiffLine[];
  isLongDiff: boolean;
  expanded: boolean;
  hiddenLineCount: number;
  onToggleExpanded: () => void;
}

function diffLineTestId(type: ParsedDiffLine["type"]) {
  if (type === "add") return "diff-line-added";
  if (type === "remove") return "diff-line-removed";
  return `diff-line-${type}`;
}

export function DiffBody({
  visibleLines,
  isLongDiff,
  expanded,
  hiddenLineCount,
  onToggleExpanded,
}: DiffBodyProps) {
  return (
    <>
      <div className="forge-diff-body">
        {visibleLines.map((line, i) => (
          <div
            key={`${i}-${line.raw}`}
            data-testid={diffLineTestId(line.type)}
            className={DIFF_LINE_CLASS[line.type]}
          >
            <span data-testid="diff-line-old-number" className="forge-diff-line-number">
              {line.oldNumber ?? ""}
            </span>
            <span data-testid="diff-line-new-number" className="forge-diff-line-number">
              {line.newNumber ?? ""}
            </span>
            <span className="forge-diff-line-code">{line.raw || " "}</span>
          </div>
        ))}
      </div>
      {isLongDiff && (
        <div className="forge-diff-footer">
          <button
            type="button"
            onClick={onToggleExpanded}
            className="forge-diff-expand"
          >
            <ChevronDown className={`size-3 transition-transform ${expanded ? "rotate-180" : ""}`} />
            {expanded ? "收起改动" : "展开完整改动"}
            {!expanded && (
              <span className="font-mono text-[10px] text-muted-foreground/70">+{hiddenLineCount} 行</span>
            )}
          </button>
        </div>
      )}
    </>
  );
}
