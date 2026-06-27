import { GitBranch } from "lucide-react";
import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "@/lib/tauri";
import { ForgeIcon } from "@/components/primitives/icon";

export function ProjectStatusDetails({
  checkpoint,
  runtime,
}: {
  checkpoint: ProjectCheckpointStatus | null;
  runtime: ProjectRuntimeStatus | null;
}) {
  return (
    <div data-forge-motion="project-status-entry" className="forge-project-status-details">
      <DetailLine label="预览状态" value={runtime?.message || "暂无"} />
      <DetailLine label="预览地址" value={runtime?.url || "暂无"} />
      <DetailLine label="预览归属" value={runtime?.working_dir || "暂无"} />
      <DetailLine label="运行命令" value={runtime?.command || "未检测到"} />
      <DetailLine label="检查点" value={checkpoint?.message || "暂无"} />
      {checkpoint?.snapshot_warning ? (
        <DetailLine label="快照提醒" value={checkpoint.snapshot_warning} />
      ) : null}
      {checkpoint?.last_checkpoint && (
        <div className="forge-project-status-commit">
          <ForgeIcon icon={GitBranch} tone="safety" contained={false} className="size-3.5" />
          <span className="truncate">{checkpoint.last_checkpoint.head}</span>
        </div>
      )}
    </div>
  );
}

function DetailLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="forge-project-status-detail-line">
      <span className="forge-project-status-detail-label">{label}</span>
      <span className="forge-project-status-detail-value">{value}</span>
    </div>
  );
}
