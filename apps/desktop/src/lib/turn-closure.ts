import type { DeliverySummary } from "@/lib/protocol";

export type DeliveryCardTone = "normal" | "warning" | "danger";
export type DeliveryPrimaryAction = "continue_fix" | "open_records" | "check_version";

export interface DeliverySummaryItem {
  label: string;
  value: string;
  kind: "preview" | "checkpoint" | "verification" | "record" | "next";
}

export interface DeliveryPrimaryActionView {
  action: DeliveryPrimaryAction;
  label: string;
  prompt?: string;
}

export interface DeliveryCardView {
  tone: DeliveryCardTone;
  items: DeliverySummaryItem[];
  primaryAction: DeliveryPrimaryActionView;
}

export function deriveDeliveryCardView(summary: DeliverySummary): DeliveryCardView {
  const verificationFailed = summary.verification_status === "failed" || summary.verification_status === "error";
  const hasPendingRecord = Boolean(summary.record_label) && summary.record_status === "pending";

  const items: DeliverySummaryItem[] = [
    { label: "预览", value: summary.preview_label, kind: "preview" },
    { label: "检查点", value: summary.checkpoint_label, kind: "checkpoint" },
  ];
  if (summary.verification_label) {
    items.push({ label: "检查", value: summary.verification_label, kind: "verification" });
  }
  if (summary.record_label) {
    items.push({ label: "自动记录", value: summary.record_label, kind: "record" });
  }
  items.push({ label: "下一步", value: summary.next_action, kind: "next" });

  if (verificationFailed) {
    return {
      tone: "danger",
      items,
      primaryAction: {
        action: "continue_fix",
        label: "继续修复",
        prompt: repairPrompt(summary),
      },
    };
  }

  if (hasPendingRecord) {
    return {
      tone: "warning",
      items,
      primaryAction: {
        action: "open_records",
        label: "查看记录",
      },
    };
  }

  return {
    tone: summary.verification_status === "skipped" ? "warning" : "normal",
    items,
    primaryAction: {
      action: "check_version",
      label: "检查这版",
      prompt: "帮我检查当前版本有没有明显问题。重点看交互、状态变化、预览可用性和下一步风险。",
    },
  };
}

function repairPrompt(summary: DeliverySummary) {
  const command = summary.verification_command?.trim();
  if (command) {
    return `继续修复刚才检查未通过的问题。请优先根据检查命令 \`${command}\` 的结果定位并修好，再重新运行检查。`;
  }
  return "继续修复刚才检查未通过的问题。请先定位失败原因，修好后重新运行检查。";
}
