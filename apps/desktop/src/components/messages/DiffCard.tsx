import { FileDiff } from "lucide-react";
import type { BlockState } from "@/lib/protocol";

export function DiffCard({ block }: { block: BlockState }) {
  const filePath = (block.metadata.file_path as string) || "";
  const diff = block.content || "";

  if (!diff) return null;

  const lines = diff.split("\n");

  return (
    <div className="mb-3">
      <div className="flex items-center gap-2 px-3 py-2 rounded-t-md border border-b-0"
        style={{ background: "#0a0a0a", borderColor: "#181818" }}>
        <FileDiff className="size-3.5" style={{ color: "#5B9BD5" }} />
        <span className="text-[11px] font-mono" style={{ color: "#999" }}>{filePath}</span>
      </div>
      <div className="overflow-auto max-h-[300px] rounded-b-md border font-mono text-[11px] leading-relaxed"
        style={{ background: "#060606", borderColor: "#181818" }}>
        {lines.map((line, i) => {
          let bg = "transparent";
          let fg = "#888";
          if (line.startsWith("+") && !line.startsWith("+++")) { bg = "rgba(74,158,107,0.08)"; fg = "#4A9E6B"; }
          else if (line.startsWith("-") && !line.startsWith("---")) { bg = "rgba(212,119,119,0.08)"; fg = "#D47777"; }
          else if (line.startsWith("@@")) { fg = "#D4A853"; }
          else if (line.startsWith("diff ")) { fg = "#5B9BD5"; }
          return (
            <div key={i} className="px-3 py-px whitespace-pre" style={{ background: bg, color: fg }}>
              {line || " "}
            </div>
          );
        })}
      </div>
    </div>
  );
}
