import { createFileTab } from "../workpanel/workPanelSelectors.ts";
import type { WorkPanelTab } from "../workpanel/workPanelTypes.ts";
import type { ProcessDigestItem } from "./conversationTurnView";

export interface ConversationProcessTarget {
  accessibleLabel: string;
  tab: WorkPanelTab;
}

export function deriveConversationProcessTarget(item: ProcessDigestItem): ConversationProcessTarget | null {
  const path = item.evidence.map(filePathFromEvidence).find((value): value is string => Boolean(value));
  if (!path) return null;
  const tab = createFileTab(path);
  return {
    accessibleLabel: `在工作面板打开 ${tab.label}`,
    tab,
  };
}

function filePathFromEvidence(block: ProcessDigestItem["evidence"][number]) {
  if (block.event_type === "diff_view") return nonEmptyString(block.metadata.file_path);
  if (block.event_type !== "tool_call" && block.event_type !== "tool_call_result") return null;
  if (!isFileTool(block.metadata.tool_name)) return null;
  const input = block.metadata.tool_input;
  if (!isRecord(input)) return null;
  return nonEmptyString(input.path)
    ?? nonEmptyString(input.file_path)
    ?? nonEmptyString(input.filename)
    ?? nonEmptyString(input.target_path);
}

function isFileTool(value: unknown) {
  return typeof value === "string"
    && ["read_file", "read", "write_file", "write", "edit"].includes(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nonEmptyString(value: unknown) {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}
