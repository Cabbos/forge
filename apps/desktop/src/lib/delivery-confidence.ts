import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";

export type DeliveryAction = "start_preview" | "open_preview" | "create_checkpoint";

export interface DeliveryConfidence {
  preview: {
    label: string;
    color: string;
    action: DeliveryAction | null;
    actionLabel: string | null;
  };
  checkpoint: {
    label: string;
    color: string;
    action: DeliveryAction | null;
    actionLabel: string | null;
  };
  nextAction: string;
}

export function getDeliveryConfidence(
  runtime: ProjectRuntimeStatus | null,
  checkpoint: ProjectCheckpointStatus | null,
): DeliveryConfidence {
  const previewRunning = runtime?.running ?? false;
  const preview = previewRunning
    ? {
        label: "预览运行中",
        color: "var(--forge-icon-safety)",
        action: runtime?.can_open ? "open_preview" as const : null,
        actionLabel: runtime?.can_open ? "打开预览" : null,
      }
    : {
        label: runtime ? "预览未运行" : "预览状态未知",
        color: "var(--forge-icon-neutral)",
        action: runtime?.can_start ? "start_preview" as const : null,
        actionLabel: runtime?.can_start ? "启动预览" : null,
      };

  const hasCheckpoint = Boolean(checkpoint?.last_checkpoint);
  const checkpointView = hasCheckpoint
    ? {
        label: checkpoint?.dirty ? "已有检查点，当前有改动" : "检查点已就绪",
        color: "var(--primary)",
        action: null,
        actionLabel: null,
      }
    : {
        label: checkpoint
          ? checkpoint.is_git_repo
            ? "还没有检查点"
            : "当前不是 Git 项目"
          : "检查点状态未知",
        color: "var(--forge-icon-neutral)",
        action: checkpoint?.is_git_repo ? "create_checkpoint" as const : null,
        actionLabel: checkpoint?.is_git_repo ? "创建检查点" : null,
      };

  return {
    preview,
    checkpoint: checkpointView,
    nextAction: nextActionFor(preview.action, checkpointView.action),
  };
}

function nextActionFor(previewAction: DeliveryAction | null, checkpointAction: DeliveryAction | null): string {
  if (previewAction === "start_preview" && checkpointAction === "create_checkpoint") {
    return "下一步：启动预览，并创建检查点。";
  }
  if (previewAction === "start_preview") return "下一步：先启动预览。";
  if (checkpointAction === "create_checkpoint") return "下一步：先创建检查点。";
  return "下一步：交付状态可以继续验收。";
}
