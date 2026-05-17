import type { KeyStatus, ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";
import type { Workspace } from "@/lib/workspaces";

export type ReadinessTone = "ready" | "warning" | "blocked" | "muted";
export type ReadinessAction = "open_settings" | "start_preview" | "create_checkpoint" | null;

export interface StartReadinessRow {
  label: string;
  value: string;
  tone: ReadinessTone;
  action: ReadinessAction;
  actionLabel: string | null;
}

export interface StartReadinessView {
  title: string;
  subtitle: string;
  issueCount: number;
  rows: StartReadinessRow[];
}

export function deriveStartReadiness(input: {
  workspace: Workspace | null;
  providerId: string;
  providerLabel: string;
  keyStatuses: KeyStatus[];
  runtime: ProjectRuntimeStatus | null;
  checkpoint: ProjectCheckpointStatus | null;
}): StartReadinessView {
  const keyStatuses = Array.isArray(input.keyStatuses) ? input.keyStatuses : [];
  const keySet = keyStatuses.some((item) => item.provider === input.providerId && item.set);
  const rows: StartReadinessRow[] = [
    {
      label: "工作空间",
      value: input.workspace ? `当前项目：${input.workspace.name}` : "还没有选择项目",
      tone: input.workspace ? "ready" : "blocked",
      action: null,
      actionLabel: null,
    },
    {
      label: "模型密钥",
      value: keySet ? `${input.providerLabel} 已配置` : `还没有配置 ${input.providerLabel}`,
      tone: keySet ? "ready" : "blocked",
      action: keySet ? null : "open_settings",
      actionLabel: keySet ? null : "打开设置",
    },
    {
      label: "预览",
      value: input.runtime?.running
        ? "已启动"
        : input.runtime?.can_start
          ? "可启动"
          : "没有检测到 dev 脚本",
      tone: input.runtime?.running || input.runtime?.can_start ? "ready" : "warning",
      action: input.runtime?.can_start && !input.runtime.running ? "start_preview" : null,
      actionLabel: input.runtime?.can_start && !input.runtime.running ? "启动预览" : null,
    },
    {
      label: "检查点",
      value: input.checkpoint?.last_checkpoint
        ? "检查点已就绪"
        : input.checkpoint?.is_git_repo
          ? "可创建"
          : "当前不是 Git 项目",
      tone: input.checkpoint?.last_checkpoint || input.checkpoint?.is_git_repo ? "ready" : "warning",
      action: input.checkpoint?.is_git_repo && !input.checkpoint.last_checkpoint ? "create_checkpoint" : null,
      actionLabel: input.checkpoint?.is_git_repo && !input.checkpoint.last_checkpoint ? "创建检查点" : null,
    },
  ];

  const issueCount = rows.filter((row) => row.tone === "blocked" || row.tone === "warning").length;
  return {
    title: "准备开始",
    subtitle: issueCount === 0 ? "可以开始做第一版小工具。" : "开始前有几项可以先确认。",
    issueCount,
    rows,
  };
}
