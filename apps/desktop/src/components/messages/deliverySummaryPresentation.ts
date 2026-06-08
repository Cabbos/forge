import type { ForgeIconTone } from "@/lib/capability-icons";
import type { DeliverySummary } from "@/lib/protocol";
import { deriveDeliveryCardView } from "@/lib/turn-closure";
import { workspaceNameFromPath } from "@/lib/workspaces";

export function deriveDeliverySummaryPresentation(value: unknown) {
  const summary = parseSummary(value);
  const view = deriveDeliveryCardView(summary);

  return {
    summary,
    view,
    projectName: summary.project_path ? workspaceNameFromPath(summary.project_path) : null,
    panelTone: messagePanelTone(view.tone),
    iconTone: deliveryTone(view.tone),
  };
}

export function messagePanelTone(tone: string): "danger" | "warning" | "default" {
  if (tone === "danger") return "danger";
  if (tone === "warning") return "warning";
  return "default";
}

export function deliveryTone(tone: string): ForgeIconTone {
  if (tone === "danger") return "danger";
  if (tone === "warning") return "safety";
  return "safety";
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
