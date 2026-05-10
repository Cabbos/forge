import { useState } from "react";
import type { BlockState } from "../../lib/protocol";

interface ToolCallCardProps {
  block: BlockState;
}

export function ToolCallCard({ block }: ToolCallCardProps) {
  const [expanded, setExpanded] = useState(true);
  const toolName = (block.metadata.tool_name as string) || "unknown";
  const toolInput = (block.metadata.tool_input as string) || "{}";
  const isError = (block.metadata.is_error as boolean) || false;

  return (
    <div
      className={`border rounded-lg overflow-hidden bg-surface-alt ${
        isError ? "border-red-300" : "border-blue-300"
      }`}
    >
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs
                   hover:bg-surface-hover transition-colors"
        aria-expanded={expanded}
      >
        <span className={isError ? "text-red-500" : "text-blue-500"}>
          {expanded ? "v" : ">"}
        </span>
        <span className="font-medium text-text-primary">
          Tool: {toolName}
        </span>
        {!block.isComplete && (
          <span className="inline-block w-2 h-2 rounded-full bg-yellow-500 animate-pulse ml-auto" />
        )}
        {isError && (
          <span className="text-red-500 text-[10px] font-medium ml-auto">
            Error
          </span>
        )}
      </button>

      {/* Content */}
      {expanded && (
        <div className="px-3 pb-3 border-t border-border">
          {/* Input */}
          <div className="mt-2">
            <p className="text-[10px] text-text-muted font-medium uppercase mb-1">
              Input
            </p>
            <pre className="!m-0 !p-2 !text-xs !bg-surface">
              {formatJSON(toolInput)}
            </pre>
          </div>

          {/* Result (if available) */}
          {block.content && (
            <div className="mt-2">
              <p className="text-[10px] text-text-muted font-medium uppercase mb-1">
                Result
              </p>
              <pre
                className={`!m-0 !p-2 !text-xs !bg-surface whitespace-pre-wrap ${
                  isError ? "!border-red-300" : ""
                }`}
              >
                {block.content}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function formatJSON(json: string): string {
  try {
    return JSON.stringify(JSON.parse(json), null, 2);
  } catch {
    return json;
  }
}
