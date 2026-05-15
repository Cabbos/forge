import { ArrowUpRight, FolderOpen, ListChecks, Sparkles, Target } from "lucide-react";
import type { ReactNode } from "react";
import { useStore } from "@/store";
import type { ProjectArchiveOverview } from "@/lib/project-archive-overview";

export function ProjectOverviewCard({ overview }: { overview: ProjectArchiveOverview }) {
  const setPendingInput = useStore((s) => s.setPendingInput);

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">项目概览</h3>
        <span className="text-[10px] text-muted-foreground/70">回来继续</span>
      </div>

      <div className="rounded-md border border-border bg-card px-3 py-3">
        <div className="flex min-w-0 items-start gap-2 border-b border-border pb-2.5">
          <FolderOpen className="mt-0.5 size-3.5 shrink-0 text-primary" />
          <div className="min-w-0">
            <div className="truncate text-xs font-medium text-foreground">{overview.projectName}</div>
            <div className="mt-0.5 truncate font-mono text-[10px] text-muted-foreground/70">
              {overview.projectPath}
            </div>
          </div>
        </div>

        <div className="space-y-2.5 py-3">
          <OverviewLine icon={<Target className="size-3" />} label="目标" value={overview.goal} />
          <OverviewLine icon={<ListChecks className="size-3" />} label="当前版本" value={overview.currentVersion} />
          <OverviewLine icon={<ArrowUpRight className="size-3" />} label="下一步" value={overview.nextStep} />
        </div>

        <div className="flex flex-wrap gap-1.5 border-t border-border pt-2.5">
          {overview.actions.map((action) => (
            <button
              key={action.id}
              type="button"
              onClick={() => setPendingInput(action.prompt)}
              className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border bg-secondary/40 px-2 text-[11px] text-foreground transition-colors hover:bg-secondary"
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
