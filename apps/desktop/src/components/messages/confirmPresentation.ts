import type { ForgeIconTone } from "@/lib/capability-icons";
import type { WriteBoundaryRiskView } from "@/lib/write-boundary";

export const kindLabels: Record<string, string> = {
  file_write: "确认改文件",
  shell_cmd: "确认执行命令",
  dangerous_cmd: "高风险命令",
  mcp_tool: "确认调用连接",
  mcp_resource_read: "确认读取连接资料",
  mcp_prompt_get: "确认使用连接提示词",
  ask_user: "需要你确认",
};

export interface ConfirmPromptViewModel {
  question: string;
  kind: string;
  kindLabel: string;
  helperText: string;
}

export function deriveConfirmPromptView(content: string, kindValue: unknown) {
  const kind = typeof kindValue === "string" && kindValue.trim().length > 0 ? kindValue : "operation";

  return {
    question: content || "是否允许这一步操作？",
    kind,
    kindLabel: kindLabels[kind] || "需要你确认",
    helperText: helperTextForKind(kind),
  };
}

export function helperTextForKind(kind: string) {
  if (kind === "dangerous_cmd") {
    return "这一步可能影响项目或本机环境。不确定时可以取消，再让 Forge 解释原因。";
  }
  if (kind === "mcp_tool") {
    return "继续后 Forge 会调用这个连接提供的工具；取消后这一步不会继续。";
  }
  if (kind === "mcp_resource_read") {
    return "继续后 Forge 会读取连接里的资料并用于本轮上下文；取消后不会读取。";
  }
  if (kind === "mcp_prompt_get") {
    return "继续后 Forge 会使用连接提供的提示词辅助本轮任务；取消后不会使用。";
  }
  return "继续后 Forge 会执行这一步；取消后这一步不会继续。";
}

export function confirmRiskColor(riskTone: WriteBoundaryRiskView) {
  if (riskTone === "high") return "var(--destructive)";
  if (riskTone === "medium") return "var(--primary)";
  return "var(--forge-icon-safety)";
}

export function confirmIconTone(riskTone: WriteBoundaryRiskView): ForgeIconTone {
  return riskTone === "high" ? "danger" : "safety";
}

export function confirmResolvedLabel(answer: boolean | null) {
  return answer ? "已继续" : "已取消";
}

export function boundaryCommandLabel(operationLabel: string) {
  if (operationLabel === "调用工具") return "工具";
  if (operationLabel === "读取资料") return "资料";
  if (operationLabel === "使用提示词") return "提示词";
  return "命令";
}
