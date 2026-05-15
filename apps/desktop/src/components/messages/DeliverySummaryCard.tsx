import { ArrowUpRight, ClipboardCheck, ExternalLink, ShieldCheck, Sparkles } from "lucide-react";
import type { ReactNode } from "react";
import { useState } from "react";
import type { BlockState, DeliverySummary } from "@/lib/protocol";
import { useStore } from "@/store";

const FOLLOW_UP_ACTIONS = [
  {
    label: "检查风险",
    icon: ShieldCheck,
    prompt: "请检查刚才的改动有没有风险、遗漏或需要我确认的地方，并按严重程度排序。",
  },
  {
    label: "继续优化",
    icon: Sparkles,
    prompt: "请基于当前结果，继续找一个最影响使用体验的问题并直接优化，最后给我验收提示词。",
  },
];

export function DeliverySummaryCard({ block }: { block: BlockState }) {
  const [loadedPrompt, setLoadedPrompt] = useState<string | null>(null);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const summary = parseSummary(block.metadata.summary);

  const loadPrompt = (prompt: string) => {
    const scopedPrompt = withTargetProject(prompt, summary.project_path);
    setPendingInput(scopedPrompt);
    setLoadedPrompt(scopedPrompt);
    window.setTimeout(() => setLoadedPrompt(null), 1200);
  };

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

      <div className="flex flex-wrap items-center gap-2 border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
        {FOLLOW_UP_ACTIONS.map((action) => {
          const Icon = action.icon;
          const prompt = withTargetProject(action.prompt, summary.project_path);
          const loaded = loadedPrompt === prompt;

          return (
            <button
              key={action.label}
              type="button"
              onClick={() => loadPrompt(action.prompt)}
              className="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] transition-colors hover:text-foreground"
              style={{
                background: loaded ? "rgba(212,168,83,0.12)" : "var(--background)",
                borderColor: loaded ? "rgba(212,168,83,0.45)" : "var(--border)",
                color: loaded ? "#D4A853" : "var(--muted-foreground)",
              }}
            >
              {loaded ? <ArrowUpRight className="size-3" /> : <Icon className="size-3" />}
              {loaded ? "已放入" : action.label}
            </button>
          );
        })}
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

function withTargetProject(prompt: string, projectPath: string | null): string {
  if (!projectPath) return prompt;
  return `${prompt}\n\n目标项目：${projectPath}`;
}

function fallbackSummary(): DeliverySummary {
  return {
    project_path: null,
    preview_label: "预览状态未知",
    checkpoint_label: "检查点状态未知",
    next_action: "下一步：继续检查交付状态。",
  };
}
