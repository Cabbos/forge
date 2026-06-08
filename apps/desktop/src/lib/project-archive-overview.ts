import type { FirstLoopDraft } from "@/lib/first-loop";
import { deriveFirstLoopDraft } from "@/lib/first-loop";
import type { BlockState, DeliverySummary, SessionState } from "@/lib/protocol";
import type { Workspace } from "@/lib/workspaces";

export type ProjectOverviewActionId = "continue_last_task" | "check_current_version" | "continue_polish";

export interface ProjectOverviewAction {
  id: ProjectOverviewActionId;
  label: string;
  prompt: string;
}

export interface ProjectArchiveOverview {
  projectName: string;
  projectPath: string;
  goal: string;
  currentVersion: string;
  nextStep: string;
  recordReview: ProjectArchiveRecordReview | null;
  actions: ProjectOverviewAction[];
}

export interface ProjectArchiveRecordReview {
  label: string;
  targetPages: string[];
}

export function deriveProjectArchiveOverview(input: {
  workspace: Workspace | null;
  session: SessionState | null;
  blocks: BlockState[];
  firstLoopDraft: FirstLoopDraft | null;
  deliverySummary?: DeliverySummary | null;
}): ProjectArchiveOverview {
  const projectPath = normalizeProjectPath(input.session?.workingDir || input.workspace?.path || "");
  const projectName = input.workspace?.name || nameFromPath(projectPath) || "当前项目";
  const latestUserMessage = latestBlock(input.blocks, "user_message")?.content.trim() ?? "";
  const latestDelivery = input.deliverySummary
    ?? parseDeliverySummary(latestBlock(input.blocks, "delivery_summary")?.metadata.summary);
  const derivedDraft = input.firstLoopDraft ?? (
    latestUserMessage ? deriveFirstLoopDraft(input.session?.id ?? "project", latestUserMessage) : null
  );

  const goal = derivedDraft?.goal || latestUserMessage || "等待你描述这个项目要做什么。";
  const currentVersion = latestDelivery
    ? `${latestDelivery.preview_label} · ${latestDelivery.checkpoint_label}`
    : derivedDraft?.scope || "还没有形成可验收版本";
  const nextStep = latestDelivery?.next_action || derivedDraft?.nextStep || "描述一个小工具，Forge 会先推进到可预览第一版。";
  const recordReview = latestDelivery?.record_label && latestDelivery.record_status === "pending"
    ? {
        label: latestDelivery.record_label,
        targetPages: latestDelivery.record_target_pages ?? [],
      }
    : null;

  return {
    projectName,
    projectPath: projectPath || "暂无项目路径",
    goal,
    currentVersion,
    nextStep,
    recordReview,
    actions: [
      {
        id: "continue_last_task",
        label: "继续上次任务",
        prompt: `继续上次任务：${goal}\n\n请先说明当前项目进展，再直接推进下一步。`,
      },
      {
        id: "check_current_version",
        label: "检查当前版本",
        prompt: "请检查当前版本是否可预览、核心动作是否能完成，并列出需要我验收的地方。",
      },
      {
        id: "continue_polish",
        label: "继续优化",
        prompt: `请基于当前版本继续优化：${nextStep}\n\n优先处理最影响使用体验的一点。`,
      },
    ],
  };
}

function latestBlock(blocks: BlockState[], eventType: string) {
  return [...blocks].reverse().find((block) => block.event_type === eventType);
}

function parseDeliverySummary(value: unknown): DeliverySummary | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Partial<Record<keyof DeliverySummary, unknown>>;
  const preview = stringValue(record.preview_label);
  const checkpoint = stringValue(record.checkpoint_label);
  if (!preview || !checkpoint) return null;

  return {
    project_path: stringValue(record.project_path),
    preview_label: preview,
    checkpoint_label: checkpoint,
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

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}

function nameFromPath(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? "";
}
