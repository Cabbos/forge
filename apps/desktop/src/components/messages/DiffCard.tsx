import { useState } from "react";
import { FileCode, ChevronDown, ChevronRight, ArrowRight } from "lucide-react";
import { cn } from "@/lib/utils";
import type { BlockState } from "@/lib/protocol";

interface DiffCardProps {
  block: BlockState;
}

export function DiffCard({ block }: DiffCardProps) {
  const [expanded, setExpanded] = useState(true);
  const filePath = (block.metadata?.file_path as string) || "unknown";
  const oldContent = (block.metadata?.old_content as string) || "";
  const newContent = block.content || "";

  // Simple line-by-line diff
  const oldLines = oldContent.split("\n");
  const newLines = newContent.split("\n");
  const maxLen = Math.max(oldLines.length, newLines.length);

  return (
    <div className="my-4 border border-border rounded-xl overflow-hidden bg-card">
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-4 py-2.5 hover:bg-muted/50 transition-colors text-left"
      >
        {expanded ? (
          <ChevronDown className="size-3.5 text-muted-foreground" />
        ) : (
          <ChevronRight className="size-3.5 text-muted-foreground" />
        )}
        <FileCode className="size-3.5 text-blue-500" />
        <span className="text-xs font-medium">{filePath}</span>
        <span className="text-[10px] text-muted-foreground">
          {oldLines.length} → {newLines.length} lines
        </span>
      </button>

      {expanded && (
        <div className="border-t border-border">
          {/* Diff content — unified view */}
          <div className="overflow-auto max-h-80">
            <table className="w-full text-xs font-mono leading-5">
              <tbody>
                {Array.from({ length: maxLen }).map((_, i) => {
                  const oldLine = oldLines[i];
                  const newLine = newLines[i];
                  const isSame = oldLine === newLine;
                  const isAdded = oldLine === undefined;
                  const isRemoved = newLine === undefined;

                  return (
                    <tr
                      key={i}
                      className={cn(
                        isRemoved && "bg-destructive/10",
                        isAdded && "bg-green-500/10",
                        !isSame && !isRemoved && !isAdded && "bg-amber-500/10"
                      )}
                    >
                      <td className="w-10 text-right pr-2 text-muted-foreground select-none border-r border-border shrink-0 align-top px-1">
                        {isRemoved ? (
                          <span>{i + 1}</span>
                        ) : (
                          <span className="text-muted-foreground/70">
                            {i + 1}
                          </span>
                        )}
                      </td>
                      <td className="w-4 text-center shrink-0 align-top">
                        {isAdded ? (
                          <span className="text-green-500">+</span>
                        ) : isRemoved ? (
                          <span className="text-destructive">-</span>
                        ) : !isSame ? (
                          <ArrowRight className="size-3 inline text-amber-500" />
                        ) : null}
                      </td>
                      <td className="w-10 text-right pr-2 text-muted-foreground select-none border-r border-border shrink-0 align-top px-1">
                        {!isRemoved && (
                          <span className="text-muted-foreground/70">
                            {i + 1}
                          </span>
                        )}
                      </td>
                      <td className="pl-2 align-top whitespace-pre-wrap break-all">
                        {isRemoved ? (
                          <span className="text-destructive line-through">{oldLine}</span>
                        ) : isAdded ? (
                          <span className="text-green-600 dark:text-green-400">{newLine}</span>
                        ) : (
                          <span
                            className={
                              isSame ? "text-foreground/80" : "text-amber-600 dark:text-amber-300"
                            }
                          >
                            {newLine}
                          </span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
