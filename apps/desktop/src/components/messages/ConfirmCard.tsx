import { useState } from "react";
import { ShieldAlert, Check, X } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";
import { useStore } from "@/store";

export function ConfirmCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const updateBlock = useStore((s) => s.updateBlock);
  const alreadyResolved = block.metadata.confirmed === true;
  const savedAnswer = block.metadata.answer as boolean | undefined;
  const [responded, setResponded] = useState(alreadyResolved);
  const [answer, setAnswer] = useState<boolean | null>(savedAnswer ?? null);
  const question = block.content || "是否允许这一步操作？";
  const kind = (block.metadata.kind as string) || "operation";
  const kindLabels: Record<string, string> = {
    file_write: "确认改文件",
    shell_cmd: "确认执行命令",
    dangerous_cmd: "高风险命令",
    ask_user: "需要你确认",
  };
  const kindLabel = kindLabels[kind] || "需要你确认";
  const helperText = kind === "dangerous_cmd"
    ? "这一步可能影响项目或本机环境。不确定时可以拒绝，再让 AI 解释原因。"
    : "同意后 AI 会继续执行这一步；拒绝后这一步不会继续。";

  const handleResponse = async (approved: boolean) => {
    setResponded(true);
    setAnswer(approved);
    try {
      await confirmResponse(block.block_id, approved);
    } catch (e) {
      console.error("confirmResponse failed:", e);
      // Revert UI on error
      setResponded(false);
      setAnswer(null);
      return;
    }
    // Persist confirmation state so it survives session reload
    if (sessionId) {
      updateBlock(sessionId, block.block_id, {
        metadata: { ...block.metadata, confirmed: true, answer: approved },
      });
    }
  };

  return (
    <div className="mb-3">
      <div className="max-w-full overflow-hidden rounded-lg border" style={{ borderColor: "rgba(212,168,83,0.2)", background: "rgba(212,168,83,0.04)" }}>
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-b" style={{ borderColor: "rgba(212,168,83,0.15)", background: "rgba(212,168,83,0.06)" }}>
          <ShieldAlert className="size-4" style={{ color: "#D4A853" }} />
          <span className="text-xs font-semibold uppercase tracking-wide" style={{ color: "#D4A853" }}>
            {kindLabel}
          </span>
        </div>

        {/* Question */}
        <div className="px-4 py-3">
          <p className="whitespace-pre-wrap text-sm leading-relaxed" style={{ color: "#E4E7EC" }}>{question}</p>
          <p className="mt-2 text-xs leading-relaxed" style={{ color: "var(--muted-foreground)" }}>{helperText}</p>
        </div>

        {/* Actions */}
        {responded ? (
          <div className="px-4 py-2.5 border-t border-border flex items-center gap-2">
            <span className={`text-xs font-medium ${answer ? "text-green-500" : "text-destructive"}`}>
              {answer ? "已同意" : "已拒绝"}
            </span>
          </div>
        ) : (
          <div className="px-4 py-2.5 border-t flex items-center gap-2" style={{ borderColor: "rgba(212,168,83,0.1)" }}>
            <button
              onClick={(e) => { e.stopPropagation(); handleResponse(true); }}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium transition-all cursor-pointer"
              style={{ background: "#D4A853", color: "#111216" }}
            >
              <Check className="size-3.5" />
              同意继续
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); handleResponse(false); }}
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md text-xs font-medium transition-all cursor-pointer"
              style={{ background: "#D47777", color: "#111216" }}
            >
              <X className="size-3.5" />
              拒绝
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
