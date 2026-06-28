import type { LucideIcon } from "lucide-react";
import {
  Archive,
  AtSign,
  BookOpen,
  BrainCircuit,
  Cable,
  Circle,
  Cpu,
  Clock3,
  Database,
  FileText,
  ListChecks,
  Puzzle,
  Search,
  SearchCode,
  Settings,
  ShieldCheck,
  Terminal,
  Workflow,
  Wrench,
  Zap,
} from "lucide-react";

export type ForgeIconTone =
  | "context"
  | "action"
  | "reasoning"
  | "safety"
  | "automation"
  | "neutral"
  | "danger";

export interface ForgeIconMeta {
  icon: LucideIcon;
  tone: ForgeIconTone;
  label: string;
}

export function commandIconMeta(command: string): ForgeIconMeta {
  if (command.includes("test") || command.includes("review")) {
    return { icon: ShieldCheck, tone: "safety", label: "检查" };
  }
  if (command.includes("explain")) {
    return { icon: BrainCircuit, tone: "reasoning", label: "解释" };
  }
  if (command.includes("docs")) {
    return { icon: BookOpen, tone: "context", label: "文档" };
  }
  if (command.includes("compact")) {
    return { icon: Archive, tone: "context", label: "压缩" };
  }
  if (command.includes("refactor")) {
    return { icon: ListChecks, tone: "reasoning", label: "整理" };
  }
  if (command.includes("fix")) {
    return { icon: Wrench, tone: "action", label: "修复" };
  }
  return { icon: Zap, tone: "action", label: "命令" };
}

export function fileReferenceIconMeta(path: string): ForgeIconMeta {
  if (path.endsWith("/")) {
    return { icon: Database, tone: "context", label: "目录" };
  }
  return { icon: FileText, tone: "context", label: "文件" };
}

export function capabilityIconMeta(kind: string): ForgeIconMeta {
  switch (kind) {
    case "skill":
      return { icon: Puzzle, tone: "action", label: "插件" };
    case "tool":
      return { icon: SearchCode, tone: "action", label: "工具" };
    case "mcp_server":
      return { icon: Cable, tone: "context", label: "连接" };
    case "provider":
      return { icon: Cpu, tone: "reasoning", label: "模型" };
    case "hook":
      return { icon: Workflow, tone: "automation", label: "自动化" };
    default:
      return { icon: Circle, tone: "neutral", label: "能力" };
  }
}

export const composerToolbarIcons = {
  file: { icon: AtSign, tone: "context", label: "引用文件" } satisfies ForgeIconMeta,
  command: { icon: Terminal, tone: "action", label: "常用请求" } satisfies ForgeIconMeta,
  search: { icon: Search, tone: "neutral", label: "搜索" } satisfies ForgeIconMeta,
  settings: { icon: Settings, tone: "neutral", label: "设置" } satisfies ForgeIconMeta,
  fast: { icon: Zap, tone: "action", label: "快速" } satisfies ForgeIconMeta,
  automation: { icon: Clock3, tone: "automation", label: "自动化" } satisfies ForgeIconMeta,
} as const;

export function capabilityEnabledTone(enabled: boolean | undefined, tone: ForgeIconTone) {
  return enabled === false ? "neutral" : tone;
}
