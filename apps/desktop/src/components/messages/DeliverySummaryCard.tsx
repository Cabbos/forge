import { ArrowUpRight, ClipboardCheck, ExternalLink, FileText, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import { useState } from "react";
import type { BlockState, DeliverySummary } from "@/lib/protocol";
import { useStore } from "@/store";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";
import { workspaceNameFromPath } from "@/lib/workspaces";
import { deriveDeliveryCardView, type DeliverySummaryItem } from "@/lib/turn-closure";

export function DeliverySummaryCard({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  const [loadedPrompt, setLoadedPrompt] = useState<string | null>(null);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const summary = parseSummary(block.metadata.summary);
  const view = deriveDeliveryCardView(summary);
  const projectName = summary.project_path ? workspaceNameFromPath(summary.project_path) : null;

  const loadPrompt = (prompt: string) => {
    setPendingInput(prompt);
    setLoadedPrompt(prompt);
    window.setTimeout(() => setLoadedPrompt(null), 1200);
  };
  const runPrimaryAction = () => {
    if (view.primaryAction.action === "open_records") {
      window.dispatchEvent(new CustomEvent("open-hub", { detail: { section: "records" } }));
      return;
    }
    if (view.primaryAction.prompt) loadPrompt(view.primaryAction.prompt);
  };
  const loaded = loadedPrompt === view.primaryAction.prompt;

  return (
    <MessagePanel tone={messagePanelTone(view.tone)}>
      <MessagePanelHeader
        icon={<ClipboardCheck className="size-4" style={{ color: toneColor(view.tone) }} />}
        title="本轮交付"
        meta={projectName ? <span>{projectName}</span> : null}
      />

      <div className="grid gap-2 px-3 py-3 text-xs sm:grid-cols-3 lg:grid-cols-5" style={{ color: "#D0D5DD" }}>
        {view.items.map((item) => (
          <SummaryItem key={`${item.kind}-${item.label}`} item={item} />
        ))}
      </div>

      <div className="flex items-center justify-end border-t px-3 py-2" style={{ borderColor: "var(--border)" }}>
        <button
          type="button"
          onClick={runPrimaryAction}
          data-session-id={sessionId}
          className="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11px] transition-colors hover:text-foreground"
          style={{
            background: loaded ? "rgba(212,168,83,0.12)" : "var(--background)",
            borderColor: loaded ? "rgba(212,168,83,0.45)" : "var(--border)",
            color: loaded ? "#D4A853" : "var(--muted-foreground)",
          }}
        >
          {loaded ? <ArrowUpRight className="size-3" /> : primaryIcon(view.primaryAction.action)}
          {loaded ? "已放入" : view.primaryAction.label}
        </button>
      </div>
    </MessagePanel>
  );
}

function SummaryItem({ item }: { item: DeliverySummaryItem }) {
  return (
    <div className="flex min-w-0 gap-2">
      <div className="mt-0.5 shrink-0">{itemIcon(item)}</div>
      <div className="min-w-0">
        <div className="text-[10px] text-muted-foreground/75">{item.label}</div>
        <div className="mt-0.5 truncate text-foreground/90">{item.value}</div>
      </div>
    </div>
  );
}

function itemIcon(item: DeliverySummaryItem): ReactNode {
  switch (item.kind) {
    case "preview":
      return <ExternalLink className="size-3.5" style={{ color: "#5B9BD5" }} />;
    case "checkpoint":
      return <ShieldCheck className="size-3.5" style={{ color: "#D4A853" }} />;
    case "verification":
      return <ClipboardCheck className="size-3.5" style={{ color: "#8BA4F9" }} />;
    case "record":
      return <FileText className="size-3.5" style={{ color: "#4A9E6B" }} />;
    case "next":
      return <ClipboardCheck className="size-3.5" style={{ color: "#4A9E6B" }} />;
  }
}

function primaryIcon(action: string): ReactNode {
  if (action === "open_records") return <FileText className="size-3" />;
  if (action === "continue_fix") return <ArrowUpRight className="size-3" />;
  return <ShieldCheck className="size-3" />;
}

function toneColor(tone: string) {
  if (tone === "danger") return "#E06C75";
  if (tone === "warning") return "#D4A853";
  return "#4A9E6B";
}

function messagePanelTone(tone: string): "danger" | "warning" | "default" {
  if (tone === "danger") return "danger";
  if (tone === "warning") return "warning";
  return "default";
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
    verification_label: stringValue(record.verification_label),
    verification_status: stringValue(record.verification_status),
    verification_command: stringValue(record.verification_command),
    record_label: stringValue(record.record_label),
    record_status: stringValue(record.record_status),
    record_target_pages: stringList(record.record_target_pages),
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function stringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0) : [];
}

function fallbackSummary(): DeliverySummary {
  return {
    project_path: null,
    preview_label: "预览状态未知",
    checkpoint_label: "检查点状态未知",
    next_action: "下一步：继续检查交付状态。",
    verification_label: null,
    verification_status: null,
    verification_command: null,
    record_label: null,
    record_status: null,
    record_target_pages: [],
  };
}
