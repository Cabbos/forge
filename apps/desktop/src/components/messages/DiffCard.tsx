import { Check, Copy, FileDiff } from "lucide-react";
import { useState } from "react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";

export function DiffCard({ block }: { block: BlockState }) {
  const [copied, setCopied] = useState(false);
  const filePath = (block.metadata.file_path as string) || "";
  const diff = block.content || "";

  if (!diff) return null;

  const lines = diff.split("\n");
  const copyDiff = async () => {
    await navigator.clipboard?.writeText(diff);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <MessagePanel>
      <MessagePanelHeader
        icon={<FileDiff className="size-3.5" style={{ color: "#5B9BD5" }} />}
        title="文件改动"
        meta={<span className="font-mono">{filePath}</span>}
        actions={(
          <button
            type="button"
            aria-label={copied ? "已复制文件改动" : "复制文件改动"}
            title={copied ? "已复制" : "复制文件改动"}
            onClick={copyDiff}
            className="inline-flex size-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
          >
            {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
          </button>
        )}
      />
      <div className="max-h-[300px] overflow-auto font-mono text-[11px] leading-relaxed" style={{ background: "var(--background)" }}>
        {lines.map((line, i) => {
          let bg = "transparent";
          let fg = "#AEB4BF";
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
    </MessagePanel>
  );
}
