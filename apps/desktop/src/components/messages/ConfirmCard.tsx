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
    <div data-testid="confirm-boundary-row" className="forge-confirm-boundary-row">
      <dt className="forge-confirm-boundary-label">{label}</dt>
      <dd className="forge-confirm-boundary-value">{children}</dd>
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
    mcp_tool: "确认调用连接",
    mcp_resource_read: "确认读取连接资料",
    mcp_prompt_get: "确认使用连接提示词",
    ask_user: "需要你确认",
  };
  const kindLabel = kindLabels[kind] || "需要你确认";
  const helperText = kind === "dangerous_cmd"
    ? "这一步可能影响项目或本机环境。不确定时可以取消，再让 Forge 解释原因。"
    : kind === "mcp_tool"
      ? "继续后 Forge 会调用这个连接提供的工具；取消后这一步不会继续。"
      : kind === "mcp_resource_read"
        ? "继续后 Forge 会读取连接里的资料并用于本轮上下文；取消后不会读取。"
        : kind === "mcp_prompt_get"
          ? "继续后 Forge 会使用连接提供的提示词辅助本轮任务；取消后不会使用。"
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
    <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
      <span className="forge-confirm-resolved" data-state={answer ? "approved" : "cancelled"}>
        {answer ? "已继续" : "已取消"}
      </span>
    </div>
  ) : (
    <div data-testid="confirm-action-bar" className="forge-confirm-action-bar">
      <button
        data-testid="confirm-approve"
        onClick={(e) => { e.stopPropagation(); handleResponse(true); }}
        className="forge-confirm-button"
        data-variant="approve"
      >
        <Check className="size-3.5" />
        继续
      </button>
      <button
        data-testid="confirm-cancel"
        onClick={(e) => { e.stopPropagation(); handleResponse(false); }}
        className="forge-confirm-button"
        data-variant="cancel"
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

        <dl data-testid="confirm-boundary-grid" className="forge-confirm-boundary-grid">
          <BoundaryLine label={boundary.targetLabel}>{boundary.workspaceLabel}</BoundaryLine>
          <BoundaryLine label="操作">{boundary.operationLabel}</BoundaryLine>
          <BoundaryLine label="影响范围">
            <span>{boundary.affectedSummary}</span>
            {boundary.affectedFiles.length > 0 ? (
              <div className="mt-1 flex flex-wrap gap-1.5">
                {boundary.affectedFiles.slice(0, 4).map((file) => (
                  <code
                    key={file}
                    className="forge-confirm-file-chip"
                  >
                    {file}
                  </code>
                ))}
              </div>
            ) : null}
          </BoundaryLine>
          <BoundaryLine label="风险">
            <span className="forge-confirm-risk" style={{ color: riskColor }}>{boundary.riskLabel}</span>
          </BoundaryLine>
          <BoundaryLine label="恢复点">{boundary.recoveryLabel}</BoundaryLine>
          {boundary.command ? (
            <BoundaryLine label={boundaryCommandLabel(boundary.operationLabel)}>
              <code className="forge-confirm-command">
                {boundary.command}
              </code>
            </BoundaryLine>
          ) : null}
          {boundary.warning ? (
            <div data-testid="confirm-warning" role="note" className="forge-confirm-warning">
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

function boundaryCommandLabel(operationLabel: string) {
  if (operationLabel === "调用工具") return "工具";
  if (operationLabel === "读取资料") return "资料";
  if (operationLabel === "使用提示词") return "提示词";
  return "命令";
}
