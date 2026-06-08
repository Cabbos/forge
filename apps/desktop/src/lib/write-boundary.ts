import type { WriteBoundary as ProtocolWriteBoundary } from "@/lib/protocol";

export type WriteBoundaryRiskView = "low" | "medium" | "high";

export interface WriteBoundaryViewModel {
  title: string;
  targetLabel: string;
  workspaceLabel: string;
  operationLabel: string;
  affectedFiles: string[];
  affectedSummary: string;
  riskLabel: string;
  riskTone: WriteBoundaryRiskView;
  recoveryLabel: string;
  command: string | null;
  warning: string | null;
}

type BoundaryRecord = Partial<ProtocolWriteBoundary> & {
  risk_level?: unknown;
  checkpoint_status?: unknown;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => stringValue(item))
    .filter((item): item is string => Boolean(item));
}

export function operationLabel(operation: unknown): string {
  const value = stringValue(operation);
  switch (value) {
    case "write":
    case "write_file":
    case "write_to_file":
      return "写入文件";
    case "edit":
    case "edit_file":
      return "编辑文件";
    case "bash":
    case "execute_command":
    case "run_shell":
    case "shell":
      return "执行命令";
    case "mcp_tool":
    case "调用工具":
      return "调用工具";
    case "mcp_resource_read":
    case "读取资料":
      return "读取资料";
    case "mcp_prompt_get":
    case "使用提示词":
      return "使用提示词";
    case "写入文件":
    case "编辑文件":
    case "修改文件":
    case "执行命令":
      return value;
    default:
      return "修改项目";
  }
}

export function riskLabel(risk: unknown): { label: string; tone: WriteBoundaryRiskView } {
  const value = stringValue(risk);
  switch (value) {
    case "low":
    case "normal":
      return { label: "低风险", tone: "low" };
    case "medium":
    case "caution":
      return { label: "需要留意", tone: "medium" };
    case "high":
      return { label: "高风险", tone: "high" };
    default:
      return { label: "需要确认", tone: "medium" };
  }
}

export function recoveryLabel(recovery: unknown, checkpointStatus: unknown): string {
  const status = stringValue(checkpointStatus);
  if (status === "ready") return "恢复点已就绪";
  if (status === "pending") return "正在准备恢复点";
  if (status === "unavailable" || status === "missing") return "暂无恢复点";

  return stringValue(recovery) ?? "继续前会保留可检查的交付状态";
}

export function parseWriteBoundary(value: unknown): WriteBoundaryViewModel | null {
  if (!isRecord(value)) return null;

  const boundary = value as BoundaryRecord;
  const workspacePath = stringValue(boundary.workspace_path) ?? "当前项目";
  const workspaceName = stringValue(boundary.workspace_name);
  const affectedFiles = stringList(boundary.affected_files)
    .map((file) => displayProjectPath(file, workspacePath));
  const impact = stringValue(boundary.impact);
  const filesLabel = affectedFiles.length > 0
    ? affectedFiles.slice(0, 3).join("、") + (affectedFiles.length > 3 ? ` 等 ${affectedFiles.length} 个文件` : "")
    : impact ?? "当前项目";
  const risk = riskLabel(boundary.risk_level ?? boundary.risk);

  return {
    title: stringValue(boundary.title) ?? "准备修改项目",
    targetLabel: stringValue(boundary.target_label) ?? "目标项目",
    workspaceLabel: workspaceName ?? workspaceDisplayName(workspacePath),
    operationLabel: operationLabel(boundary.operation),
    affectedFiles,
    affectedSummary: affectedFiles.length > 0 ? `${affectedFiles.length} 个文件 · ${filesLabel}` : filesLabel,
    riskLabel: risk.label,
    riskTone: risk.tone,
    recoveryLabel: recoveryLabel(boundary.recovery, boundary.checkpoint_status),
    command: stringValue(boundary.command),
    warning: stringValue(boundary.warning),
  };
}

function workspaceDisplayName(path: string) {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : "当前项目";
}

function displayProjectPath(path: string, workspacePath: string) {
  const normalizedPath = path.replace(/\\/g, "/");
  const normalizedWorkspace = workspacePath.replace(/\\/g, "/").replace(/\/+$/, "");
  if (normalizedWorkspace && normalizedPath.startsWith(`${normalizedWorkspace}/`)) {
    return normalizedPath.slice(normalizedWorkspace.length + 1);
  }
  if (normalizedPath.startsWith("/") || normalizedPath.startsWith("~")) {
    return workspaceDisplayName(normalizedPath) || "项目外文件";
  }
  return path;
}
