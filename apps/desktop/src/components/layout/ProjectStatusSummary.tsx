import { GitBranch, Play, type LucideIcon } from "lucide-react";
import type { DeliveryConfidence } from "@/lib/delivery-confidence";
import { ForgeIcon } from "@/components/primitives/icon";
import type { ForgeIconTone } from "@/lib/capability-icons";

export function ProjectStatusSummary({ delivery }: { delivery: DeliveryConfidence }) {
  return (
    <div data-testid="project-status-summary" data-forge-motion="project-status-entry" className="forge-project-status-summary">
      <ProjectStatusMetric
        icon={Play}
        iconTone="action"
        color={delivery.preview.color}
        label="预览"
        value={delivery.preview.label}
      />
      <ProjectStatusMetric
        icon={GitBranch}
        iconTone="safety"
        color={delivery.checkpoint.color}
        label="检查点"
        value={delivery.checkpoint.label}
      />
    </div>
  );
}

function ProjectStatusMetric({
  icon,
  iconTone,
  color,
  label,
  value,
}: {
  icon: LucideIcon;
  iconTone: ForgeIconTone;
  color: string;
  label: string;
  value: string;
}) {
  return (
    <div data-testid="project-status-metric" className="forge-project-status-metric">
      <ForgeIcon icon={icon} tone={iconTone} contained={false} className="size-3.5" />
      <div className="min-w-0">
        <div className="forge-project-status-label">{label}</div>
        <div className="forge-project-status-value">{value}</div>
      </div>
      <span className="forge-project-status-dot" style={{ backgroundColor: color, color }} aria-hidden="true" />
    </div>
  );
}
