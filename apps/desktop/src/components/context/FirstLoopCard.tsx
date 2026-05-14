import { ArrowRight, MousePointerClick, Target } from "lucide-react";
import type { ReactNode } from "react";
import type { FirstLoopDraft } from "@/lib/first-loop";
import { FIRST_LOOP_STANDARD } from "@/lib/first-loop";

export function FirstLoopCard({ draft }: { draft: FirstLoopDraft | null }) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">第一版</h3>
        <span className="text-[10px] text-muted-foreground/70">小工具闭环</span>
      </div>

      <div className="rounded-md border border-border bg-card px-3 py-3">
        {!draft ? (
          <div className="space-y-2 text-xs text-muted-foreground">
            <div className="flex items-center gap-2 text-foreground/85">
              <Target className="size-3.5 text-primary" />
              描述一个小工具后，这里会收拢目标、第一版范围和下一步。
            </div>
            <div className="flex flex-wrap gap-1.5 pt-1 text-[10px]">
              {FIRST_LOOP_STANDARD.split("、").map((item) => (
                <span key={item} className="rounded border border-border bg-background/50 px-1.5 py-0.5 text-muted-foreground">
                  {item}
                </span>
              ))}
            </div>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex flex-wrap gap-1.5">
              {FIRST_LOOP_STANDARD.split("、").map((item) => (
                <span
                  key={item}
                  className="inline-flex items-center gap-1 rounded border border-primary/25 bg-primary/10 px-1.5 py-0.5 text-[10px] text-primary"
                >
                  <MousePointerClick className="size-3" />
                  {item}
                </span>
              ))}
            </div>

            <FirstLoopRow label="目标" value={draft.goal} />
            <FirstLoopRow label="第一版范围" value={draft.scope} />
            <FirstLoopRow label="下一步" value={draft.nextStep} icon={<ArrowRight className="size-3" />} />
            <FirstLoopRow label="标准" value={FIRST_LOOP_STANDARD} />
          </div>
        )}
      </div>
    </section>
  );
}

function FirstLoopRow({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon?: ReactNode;
}) {
  return (
    <div className="grid grid-cols-[64px_minmax(0,1fr)] gap-2 text-[11px] leading-relaxed">
      <span className="text-muted-foreground/60">{label}</span>
      <span className="flex min-w-0 items-start gap-1.5 break-words text-foreground/85">
        {icon && <span className="mt-0.5 shrink-0 text-primary">{icon}</span>}
        {value}
      </span>
    </div>
  );
}
