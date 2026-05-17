import { ArrowUpRight, ClipboardCheck, ExternalLink, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import { useState } from "react";
import type { BlockState, DeliverySummary } from "@/lib/protocol";
import { useStore } from "@/store";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { workspaceNameFromPath } from "@/lib/workspaces";

const FOLLOW_UP_ACTIONS = [
  {
    label: "检查这版",
    icon: ShieldCheck,
    prompt: "帮我检查当前版本有没有明显问题。重点看交互、状态变化、预览可用性和下一步风险。",
  },
];

export function DeliverySummaryCard({ block }: { block: BlockState }) {
  const [loadedPrompt, setLoadedPrompt] = useState<string | null>(null);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const summary = parseSummary(block.metadata.summary);
  const projectName = summary.project_path ? workspaceNameFromPath(summary.project_path) : null;

  const loadPrompt = (prompt: string) => {
    setPendingInput(prompt);
    setLoadedPrompt(prompt);
    window.setTimeout(() => setLoadedPrompt(null), 1200);
  };

  return (
    <MessagePanel>
      <MessagePanelHeader
        icon={<ClipboardCheck className="size-4" style={{ color: "#D4A853" }} />}
        title="本轮交付"
        meta={projectName ? <span title={summary.project_path ?? undefined}>{projectName}</span> : null}
      />

      <div className="grid gap-2 px-3 py-3 text-xs sm:grid-cols-3" style={{ color: "#D0D5DD" }}>
        <SummaryItem icon={<ExternalLink className="size-3.5" style={{ color: "#5B9BD5" }} />} label="预览" value={summary.preview_label} />
        <SummaryItem icon={<ShieldCheck className="size-3.5" style={{ color: "#D4A853" }} />} label="检查点" value={summary.checkpoint_label} />
        <SummaryItem icon={<ClipboardCheck className="size-3.5" style={{ color: "#4A9E6B" }} />} label="下一步" value={summary.next_action} />
      </div>

      <div className="flex items-center justify-end border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
        {FOLLOW_UP_ACTIONS.map((action) => {
          const Icon = action.icon;
          const loaded = loadedPrompt === action.prompt;

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
    </MessagePanel>
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
