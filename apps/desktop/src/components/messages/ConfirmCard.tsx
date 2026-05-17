import { useState } from "react";
import type { ReactNode } from "react";
import { ShieldAlert, Check, X } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { confirmResponse } from "@/lib/tauri";
import { parseWriteBoundary } from "@/lib/write-boundary";
import { useStore } from "@/store";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";

function BoundaryLine({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1 py-2 sm:grid-cols-[88px_1fr] sm:gap-3">
      <dt className="text-xs leading-relaxed text-muted-foreground">{label}</dt>
      <dd className="min-w-0 text-sm leading-relaxed text-foreground">{children}</dd>
    </div>
  );
}

export function ConfirmCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const updateBlock = useStore((s) => s.updateBlock);
  const alreadyResolved = block.metadata.confirmed === true;
  const savedAnswer = block.metadata.answer as boolean | undefined;
  const [responded, setResponded] = useState(alreadyResolved);
  const [answer, setAnswer] = useState<boolean | null>(savedAnswer ?? null);
  const boundary = parseWriteBoundary(block.metadata.boundary);
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
    ? "这一步可能影响项目或本机环境。不确定时可以取消，再让 Forge 解释原因。"
    : "继续后 Forge 会执行这一步；取消后这一步不会继续。";

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

  const actions = responded ? (
    <div className="flex items-center gap-2 border-t border-border px-3 py-2">
      <span className={`text-xs font-medium ${answer ? "text-green-500" : "text-destructive"}`}>
        {answer ? "已继续" : "已取消"}
      </span>
    </div>
  ) : (
    <div className="flex items-center gap-2 border-t px-3 py-2" style={{ borderColor: "rgba(212,168,83,0.1)" }}>
      <button
        onClick={(e) => { e.stopPropagation(); handleResponse(true); }}
        className="inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-colors"
        style={{ background: "#D4A853", color: "#111216" }}
      >
        <Check className="size-3.5" />
        继续
      </button>
      <button
        onClick={(e) => { e.stopPropagation(); handleResponse(false); }}
        className="inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md border px-2.5 text-xs font-medium transition-colors hover:text-foreground"
        style={{ borderColor: "rgba(212,119,119,0.32)", color: "#D47777" }}
      >
        <X className="size-3.5" />
        取消
      </button>
    </div>
  );

  if (boundary) {
    const riskColor = boundary.riskTone === "high"
      ? "#F87171"
      : boundary.riskTone === "medium"
        ? "#D4A853"
        : "#6EE7B7";

    return (
      <MessagePanel tone={boundary.riskTone === "high" ? "danger" : "warning"}>
        <MessagePanelHeader
          icon={<ShieldAlert className="size-4" style={{ color: "#D4A853" }} />}
          title={boundary.title}
          meta="继续前确认改动范围"
        />

        <dl className="px-3 py-2">
          <BoundaryLine label="目标项目">{boundary.workspaceLabel}</BoundaryLine>
          <BoundaryLine label="操作">{boundary.operationLabel}</BoundaryLine>
          <BoundaryLine label="影响范围">
            <span>{boundary.affectedSummary}</span>
            {boundary.affectedFiles.length > 0 ? (
              <div className="mt-1 flex flex-wrap gap-1.5">
                {boundary.affectedFiles.slice(0, 4).map((file) => (
                  <code
                    key={file}
                    className="max-w-full truncate rounded border border-border bg-muted px-1.5 py-0.5 text-xs text-foreground"
                  >
                    {file}
                  </code>
                ))}
              </div>
            ) : null}
          </BoundaryLine>
          <BoundaryLine label="风险">
            <span className="font-medium" style={{ color: riskColor }}>{boundary.riskLabel}</span>
          </BoundaryLine>
          <BoundaryLine label="恢复点">{boundary.recoveryLabel}</BoundaryLine>
          {boundary.command ? (
            <BoundaryLine label="命令">
              <code className="block max-w-full overflow-x-auto rounded border border-border bg-muted px-2 py-1 text-xs text-foreground">
                {boundary.command}
              </code>
            </BoundaryLine>
          ) : null}
          {boundary.warning ? (
            <div className="mt-2 rounded-md border px-3 py-2 text-xs leading-relaxed" style={{ borderColor: "rgba(248,113,113,0.22)", background: "rgba(248,113,113,0.08)", color: "#FCA5A5" }}>
              {boundary.warning}
            </div>
          ) : null}
        </dl>

        {actions}
      </MessagePanel>
    );
  }

  return (
    <MessagePanel tone="warning">
      <MessagePanelHeader
        icon={<ShieldAlert className="size-4" style={{ color: "#D4A853" }} />}
        title={kindLabel}
        meta="继续前需要你确认"
      />
      <div className="px-3 py-2.5">
        <p className="whitespace-pre-wrap text-sm leading-relaxed text-foreground">{question}</p>
        <p className="mt-2 text-xs leading-relaxed text-muted-foreground">{helperText}</p>
      </div>

      {actions}
    </MessagePanel>
  );
}
