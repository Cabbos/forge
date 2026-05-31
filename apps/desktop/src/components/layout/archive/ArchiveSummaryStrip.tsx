import type { ReactNode } from "react";
import { ClipboardCheck, FolderOpen, Layers3 } from "lucide-react";
import type { DeliverySummary } from "@/lib/protocol";
import { deriveProjectArchiveOverview } from "@/lib/project-archive-overview";

export function ArchiveSummaryStrip({
  contextCount,
  deliverySummary,
  overview,
}: {
  contextCount: number;
  deliverySummary: DeliverySummary | null | undefined;
  overview: ReturnType<typeof deriveProjectArchiveOverview>;
}) {
  const recordValue = deliverySummary?.record_status === "pending"
    ? "待确认"
    : deliverySummary?.record_status === "accepted"
      ? "已记录"
      : "未请求";

  return (
    <section data-testid="project-archive-summary-strip" className="forge-archive-summary-strip" aria-label="项目档案摘要">
      <ArchiveSummaryItem
        icon={<FolderOpen className="size-3.5" />}
        label="项目"
        value={overview.projectName}
        title={overview.projectPath}
      />
      <ArchiveSummaryItem
        icon={<Layers3 className="size-3.5" />}
        label="上下文"
        value={contextCount > 0 ? `${contextCount} 个已加入` : "未加入"}
      />
      <ArchiveSummaryItem
        icon={<ClipboardCheck className="size-3.5" />}
        label="记录"
        value={recordValue}
        title={deliverySummary?.checkpoint_label}
      />
    </section>
  );
}

function ArchiveSummaryItem({
  detail,
  icon,
  label,
  title,
  value,
}: {
  detail?: string;
  icon: ReactNode;
  label: string;
  title?: string;
  value: string;
}) {
  return (
    <div className="forge-archive-summary-item" title={title}>
      <span className="forge-archive-summary-icon">{icon}</span>
      <span className="forge-archive-summary-copy">
        <span className="forge-archive-summary-label">{label}</span>
        <span className="forge-archive-summary-value">{value}</span>
        {detail ? <span className="forge-archive-summary-detail">{detail}</span> : null}
      </span>
    </div>
  );
}
