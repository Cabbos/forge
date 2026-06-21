import type { KeyStatus, ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";
import { deriveProviderEvidenceSummary, type ProviderDefinition } from "./providers.ts";
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
  provider?: ProviderDefinition | null;
  model?: string | null;
  keyStatuses: KeyStatus[];
  runtime: ProjectRuntimeStatus | null;
  checkpoint: ProjectCheckpointStatus | null;
}): StartReadinessView {
  const keyStatuses = Array.isArray(input.keyStatuses) ? input.keyStatuses : [];
  const providerRequiresApiKey = input.provider?.requiresApiKey !== false;
  const keySet = providerRequiresApiKey
    ? keyStatuses.some((item) => item.provider === input.providerId && item.set)
    : true;
  const workspaceBlocked = !input.workspace;
  const keyBlocked = providerRequiresApiKey && !keySet;
  const selectedModel = input.model?.trim() || input.provider?.defaultModel?.trim() || "";
  const providerModels = input.provider?.models ?? [];
  const modelKnown = Boolean(selectedModel) && (
    Boolean(input.provider?.customModels) ||
    providerModels.some((model) => model.id === selectedModel)
  );
  const modelWarning = Boolean(selectedModel) && !modelKnown;
  const evidenceSummary = input.provider ? deriveProviderEvidenceSummary(input.provider) : null;
  const evidenceBlocked = evidenceSummary?.tone === "blocked";
  const hasCheckpoint = Boolean(input.checkpoint?.last_checkpoint);
  const restorableCheckpoint = Boolean(input.checkpoint?.restorable);
  const rows: StartReadinessRow[] = [
    {
      label: "当前项目",
      value: input.workspace ? `当前项目：${input.workspace.name}` : "还没有选择项目",
      tone: input.workspace ? "ready" : "blocked",
      action: null,
      actionLabel: null,
    },
    {
      label: "模型密钥",
      value: providerRequiresApiKey
        ? keySet
          ? `${input.providerLabel} 已配置`
          : `还没有配置 ${input.providerLabel}`
        : `${input.providerLabel} 不需要密钥`,
      tone: keySet ? "ready" : "blocked",
      action: keySet ? null : "open_settings",
      actionLabel: keySet ? null : "打开设置",
    },
    {
      label: "模型",
      value: selectedModel
        ? modelKnown
          ? `${selectedModel} 可用`
          : `${selectedModel} 不在 ${input.providerLabel} 模型目录`
        : "还没有选择模型",
      tone: selectedModel ? modelWarning ? "warning" : "ready" : "warning",
      action: null,
      actionLabel: null,
    },
    {
      label: "Provider 证据",
      value: evidenceSummary
        ? `${evidenceSummary.label}：${evidenceSummary.detail}`
        : "Provider 证据未知",
      tone: evidenceSummary?.tone ?? "warning",
      action: evidenceBlocked ? "open_settings" : null,
      actionLabel: evidenceBlocked ? "打开设置" : null,
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
      value: hasCheckpoint
        ? restorableCheckpoint
          ? "检查点已就绪"
          : "检查点不可回退"
        : input.checkpoint?.is_git_repo
          ? "可创建"
          : "当前不是 Git 项目",
      tone: restorableCheckpoint || (input.checkpoint?.is_git_repo && !hasCheckpoint) ? "ready" : "warning",
      action: input.checkpoint?.is_git_repo && !restorableCheckpoint ? "create_checkpoint" : null,
      actionLabel: input.checkpoint?.is_git_repo && !restorableCheckpoint
        ? hasCheckpoint
          ? "重新创建检查点"
          : "创建检查点"
        : null,
    },
  ];

  const issueCount = rows.filter((row) => row.tone === "blocked" || row.tone === "warning").length;
  return {
    title: workspaceBlocked
      ? "选择一个项目开始"
      : keyBlocked
        ? "需要配置模型密钥"
        : evidenceBlocked
          ? "Provider 检测失败"
          : "准备开始",
    subtitle: issueCount === 0
      ? "可以开始做第一版小工具。"
      : keyBlocked
        ? `添加 ${input.providerLabel} 密钥后就可以发送第一句话。`
        : evidenceBlocked
          ? "打开设置重新检测 provider。"
          : "开始前有几项可以先确认。",
    issueCount,
    rows,
  };
}
