import type { SelectedContextMemory, SelectedForgeWikiPage } from "@/lib/protocol";

export type ActiveContextKind = "memory" | "forge_wiki_page";

export interface ActiveContextItem {
  id: string;
  kind: ActiveContextKind;
  title: string;
  summary: string;
  reason: string;
  injected: boolean;
  score?: number;
  sourceLabel: string;
  sourcePath?: string;
}

export function getActiveContextItems(
  memories: SelectedContextMemory[],
  pages: SelectedForgeWikiPage[],
): ActiveContextItem[] {
  const memoryItems = memories.map((memory): ActiveContextItem => ({
    id: memory.memory_id,
    kind: "memory",
    title: memory.title,
    summary: memory.body,
    reason: memory.reason,
    injected: memory.injected,
    score: memory.score,
    sourceLabel: memoryCategoryLabel(memory.category),
  }));

  const pageItems = pages.map((page): ActiveContextItem => ({
    id: page.page_id,
    kind: "forge_wiki_page",
    title: page.title,
    summary: page.summary,
    reason: page.reason,
    injected: page.injected,
    score: page.score,
    sourceLabel: "项目记录",
    sourcePath: page.path,
  }));

  return [...memoryItems, ...pageItems].sort(
    (a, b) => Number(b.injected) - Number(a.injected) || (b.score ?? 0) - (a.score ?? 0),
  );
}

export function countInjectedContext(items: ActiveContextItem[]): number {
  return items.filter((item) => item.injected).length;
}

export function activeContextSummary(items: ActiveContextItem[]): string {
  const injected = countInjectedContext(items);
  if (items.length === 0) return "本轮没有带入额外档案";
  if (injected === 0) return `找到 ${items.length} 条相关档案`;
  return `已参考 ${injected} 条档案`;
}

function memoryCategoryLabel(category: SelectedContextMemory["category"]): string {
  switch (category) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目信息";
    case "decision":
      return "已定方案";
    case "task_state":
      return "当前进度";
  }
}
