import type { McpContextPromptArgument, McpContextSelection } from "@/lib/tauri";
import { type McpContextSources } from "@/lib/tauri";
import type { McpContextStatus } from "@/lib/protocol";

export type ParseStatus = "pending" | "parsed" | "failed" | "available" | "read_failed";

export interface ContextFile {
  id: string;
  name: string;
  type: string;
  status: ParseStatus;
  inContext: boolean;
  selection?: McpContextSelection;
  promptArguments?: McpContextPromptArgument[];
  sourceLabel?: string;
  description?: string;
  statusMessage?: string | null;
}

export function statusLabel(status: ParseStatus) {
  switch (status) {
    case "read_failed":
      return "读取失败";
    case "available":
      return "可用";
    case "pending":
      return "解析中";
    case "parsed":
      return "已解析";
    case "failed":
      return "解析失败";
  }
}

export function statusClass(status: ParseStatus) {
  switch (status) {
    case "read_failed":
      return "text-destructive";
    case "available":
      return "text-emerald-400";
    case "pending":
      return "text-primary";
    case "parsed":
      return "text-emerald-400";
    case "failed":
      return "text-destructive";
  }
}

export function buildContextMaterials(
  files: ContextFile[],
  sources: McpContextSources,
  selected: McpContextSelection[],
  statuses: Map<string, McpContextStatus> | null,
): ContextFile[] {
  const connectorResources = sources.resources.map((resource): ContextFile => {
    const id = `mcp-resource:${resource.server_id}:${resource.uri}`;
    const selection: McpContextSelection = {
      kind: "resource",
      server_id: resource.server_id,
      uri: resource.uri,
      name: resource.name || resource.uri,
      description: resource.description,
      mime_type: resource.mime_type,
    };
    const status = statuses?.get(id) ?? null;
    return {
      id,
      name: selection.name,
      type: compactResourceType(resource.mime_type),
      status: contextFileStatus(status),
      inContext: selected.some((item) => sameContextSelection(item, selection)),
      selection,
      sourceLabel: `连接资料 · ${resource.server_id}`,
      description: resource.description,
      statusMessage: status?.status === "failed" ? status.message ?? null : null,
    };
  });
  const connectorPrompts = sources.prompts.map((prompt): ContextFile => {
    const id = `mcp-prompt:${prompt.server_id}:${prompt.name}`;
    const selection: McpContextSelection = {
      kind: "prompt",
      server_id: prompt.server_id,
      name: prompt.name,
      description: prompt.description,
    };
    const status = statuses?.get(id) ?? null;
    return {
      id,
      name: prompt.name,
      type: "提示词",
      status: contextFileStatus(status),
      inContext: selected.some((item) => sameContextSelection(item, selection)),
      selection,
      promptArguments: prompt.arguments,
      sourceLabel: `连接提示词 · ${prompt.server_id}`,
      description: prompt.description,
      statusMessage: status?.status === "failed" ? status.message ?? null : null,
    };
  });

  return [...files, ...connectorResources, ...connectorPrompts];
}

export function contextFileStatus(status: McpContextStatus | null): ParseStatus {
  if (!status) return "available";
  return status.status === "failed" ? "read_failed" : "parsed";
}

export function sameContextSelection(a: McpContextSelection, b: McpContextSelection) {
  if (a.kind !== b.kind || a.server_id !== b.server_id) return false;
  return a.kind === "resource" && b.kind === "resource"
    ? a.uri === b.uri
    : a.kind === "prompt" && b.kind === "prompt" && a.name === b.name;
}

export function compactResourceType(mimeType: string | null) {
  if (!mimeType) return "资料";
  if (mimeType.includes("markdown")) return "md";
  if (mimeType.includes("pdf")) return "pdf";
  if (mimeType.includes("json")) return "json";
  if (mimeType.includes("text")) return "txt";
  return "资料";
}
