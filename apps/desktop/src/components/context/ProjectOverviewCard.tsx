import { ArrowUpRight, FileText, FolderOpen, ListChecks, Sparkles, Target } from "lucide-react";
import type { ReactNode } from "react";
import { useStore } from "@/store";
import type { ProjectArchiveOverview } from "@/lib/project-archive-overview";

export function ProjectOverviewCard({ overview }: { overview: ProjectArchiveOverview }) {
  const setPendingInput = useStore((s) => s.setPendingInput);
  const openRecords = () => {
    window.dispatchEvent(new CustomEvent("open-hub", { detail: { section: "records" } }));
  };

  return (
    <section>
      <div className="forge-section-head">
        <h3 className="forge-section-title">项目概览</h3>
        <span className="forge-section-meta">回来继续</span>
      </div>

      <div className="forge-surface px-3 py-3">
        <div className="flex min-w-0 items-start gap-2 border-b border-border pb-2.5">
          <FolderOpen className="mt-0.5 size-3.5 shrink-0 text-primary" />
          <div className="min-w-0" title={overview.projectPath}>
            <div className="truncate text-xs font-medium text-foreground">{overview.projectName}</div>
          </div>
        </div>

        <div className="space-y-2.5 py-3">
          <OverviewLine icon={<Target className="size-3" />} label="目标" value={overview.goal} />
          <OverviewLine icon={<ListChecks className="size-3" />} label="当前版本" value={overview.currentVersion} />
          <OverviewLine icon={<ArrowUpRight className="size-3" />} label="下一步" value={overview.nextStep} />
          {overview.recordReview ? (
            <div className="grid grid-cols-[68px_minmax(0,1fr)] gap-2 border-t border-border pt-2.5 text-[11px] leading-relaxed">
              <span className="flex items-start gap-1.5 pt-0.5 text-muted-foreground/60">
                <FileText className="size-3 text-muted-foreground/70" />
                自动记录
              </span>
              <div className="min-w-0 space-y-1.5">
                <div className="break-words text-foreground/85">{overview.recordReview.label}</div>
                {overview.recordReview.targetPages.length > 0 ? (
                  <div className="truncate text-[10px] text-muted-foreground/65" title={overview.recordReview.targetPages.join(", ")}>
                    {overview.recordReview.targetPages.join(", ")}
                  </div>
                ) : null}
                <button type="button" onClick={openRecords} className="forge-action">
                  <FileText className="size-3" />
                  查看记录
                </button>
              </div>
            </div>
          ) : null}
        </div>

        <div className="flex flex-wrap gap-1.5 border-t border-border pt-2.5">
          {overview.actions.map((action) => (
            <button
              key={action.id}
              type="button"
              onClick={() => setPendingInput(action.prompt)}
              className="forge-action"
            >
              {action.id === "continue_polish" ? <Sparkles className="size-3" /> : <ArrowUpRight className="size-3" />}
              {action.label}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}

function OverviewLine({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="grid grid-cols-[68px_minmax(0,1fr)] gap-2 text-[11px] leading-relaxed">
      <span className="flex items-center gap-1.5 text-muted-foreground/60">
        <span className="text-muted-foreground/70">{icon}</span>
        {label}
      </span>
      <span className="min-w-0 break-words text-foreground/85">{value}</span>
    </div>
  );
}
