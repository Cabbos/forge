import { ClipboardCheck, ExternalLink, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import type { BlockState, DeliverySummary } from "@/lib/protocol";

export function DeliverySummaryCard({ block }: { block: BlockState }) {
  const summary = parseSummary(block.metadata.summary);

  return (
    <div className="mb-4 max-w-[760px] rounded-lg border" style={{ background: "var(--card)", borderColor: "var(--border)" }}>
      <div className="flex items-center gap-2 border-b px-3 py-2" style={{ borderColor: "var(--border)" }}>
        <ClipboardCheck className="size-4" style={{ color: "#D4A853" }} />
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">本轮交付</div>
          {summary.project_path && (
            <div className="truncate font-mono text-[10px] text-muted-foreground/75">
              {summary.project_path}
            </div>
          )}
        </div>
      </div>

      <div className="grid gap-2 px-3 py-3 text-xs sm:grid-cols-3" style={{ color: "#D0D5DD" }}>
        <SummaryItem icon={<ExternalLink className="size-3.5" style={{ color: "#5B9BD5" }} />} label="预览" value={summary.preview_label} />
        <SummaryItem icon={<ShieldCheck className="size-3.5" style={{ color: "#D4A853" }} />} label="检查点" value={summary.checkpoint_label} />
        <SummaryItem icon={<ClipboardCheck className="size-3.5" style={{ color: "#4A9E6B" }} />} label="下一步" value={summary.next_action} />
      </div>
    </div>
  );
}

function SummaryItem({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="flex min-w-0 gap-2">
      <div className="mt-0.5 shrink-0">{icon}</div>
      <div className="min-w-0">
        <div className="text-[10px] text-muted-foreground/75">{label}</div>
        <div className="mt-0.5 truncate text-foreground/90">{value}</div>
      </div>
    </div>
  );
}

function parseSummary(value: unknown): DeliverySummary {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return fallbackSummary();
  }

  const record = value as Partial<Record<keyof DeliverySummary, unknown>>;
  return {
    project_path: stringValue(record.project_path),
    preview_label: stringValue(record.preview_label) ?? "预览状态未知",
    checkpoint_label: stringValue(record.checkpoint_label) ?? "检查点状态未知",
    next_action: stringValue(record.next_action) ?? "下一步：继续检查交付状态。",
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function fallbackSummary(): DeliverySummary {
  return {
    project_path: null,
    preview_label: "预览状态未知",
    checkpoint_label: "检查点状态未知",
    next_action: "下一步：继续检查交付状态。",
  };
}
