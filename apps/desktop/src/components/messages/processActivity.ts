import type { BlockState } from "@/lib/protocol";

export type ProcessActivityState = "error" | "running" | "done";

export interface ToolCounts {
  /** Unique tool_call blocks (deduped by block_id). */
  totalTools: number;
  /** Tool_call blocks where metadata.is_error is truthy. */
  failedTools: number;
  /** Unique shell blocks not classified as checks. */
  shellCommands: number;
  /** Shell blocks whose command matches build/test/check/lint. */
  shellChecks: number;
  /** Shell blocks with a non-zero exit_code. */
  failedShells: number;
  /** Tool-call count keyed by tool_name (deduped by block_id). */
  perTool: Record<string, number>;
  /** Most-frequently-called tool name, or null when empty. */
  topTool: string | null;
}

/**
 * Pure helper: derive tool / shell counts from a list of blocks.
 * Deduplicates by block_id so replay / update does not overcount.
 */
export function deriveToolCounts(blocks: BlockState[]): ToolCounts {
  const seen = new Set<string>();
  const perTool: Record<string, number> = {};
  let failedTools = 0;
  let shellCommands = 0;
  let shellChecks = 0;
  let failedShells = 0;

  for (const block of blocks) {
    if (seen.has(block.block_id)) continue;
    seen.add(block.block_id);

    if (block.event_type === "shell") {
      const command = String(block.metadata.command ?? "");
      if (/(build|test|check|lint)/i.test(command)) {
        shellChecks += 1;
      } else {
        shellCommands += 1;
      }
      const exitCode = block.metadata.exit_code as number | undefined;
      if (exitCode !== undefined && exitCode !== 0) {
        failedShells += 1;
      }
    } else if (block.event_type === "tool_call" || block.event_type === "tool_call_result") {
      const toolName = String(block.metadata.tool_name ?? "unknown");
      perTool[toolName] = (perTool[toolName] || 0) + 1;
      if (block.metadata.is_error) {
        failedTools += 1;
      }
    }
  }

  const totalTools = Object.values(perTool).reduce((a, b) => a + b, 0);
  const topEntries = Object.entries(perTool).sort((a, b) => b[1] - a[1]);
  const topTool = topEntries[0]?.[0] ?? null;

  return {
    totalTools,
    failedTools,
    shellCommands,
    shellChecks,
    failedShells,
    perTool,
    topTool,
  };
}

export function deriveToolActivityView(blocks: BlockState[]) {
  const hasError = blocks.some(isProcessBlockError);
  const isRunning = blocks.some((block) => !block.isComplete);
  const state: ProcessActivityState = hasError ? "error" : isRunning ? "running" : "done";
  const counts = deriveToolCounts(blocks);

  return {
    hasError,
    isRunning,
    label: processActivityLabel(state, blocks.length),
    state,
    summaryItems: summarizeActivity(blocks, counts),
    tone: processActivityTone(state),
    counts,
  };
}

function isProcessBlockError(block: BlockState) {
  if (block.event_type === "shell") {
    const exitCode = block.metadata.exit_code as number | undefined;
    return exitCode !== undefined && exitCode !== 0;
  }
  return Boolean(block.metadata.is_error ?? false);
}

function processActivityLabel(state: ProcessActivityState, count: number) {
  if (state === "error") return `处理遇到问题 · ${count} 步`;
  if (state === "running") return `正在处理 · ${count} 步`;
  return `过程已收起 · ${count} 步`;
}

function processActivityTone(state: ProcessActivityState) {
  return state === "error" ? "error" : "default";
}

function summarizeActivity(blocks: BlockState[], counts: ToolCounts) {
  const categoryCounts = blocks.reduce(
    (summary, block) => {
      if (block.event_type === "shell") {
        const command = String(block.metadata.command ?? "");
        if (/(build|test|check|lint)/i.test(command)) summary.checks += 1;
        else summary.commands += 1;
        return summary;
      }

      const toolName = String(block.metadata.tool_name ?? "");
      if (["read_file", "read"].includes(toolName)) summary.reads += 1;
      else if (["write_file", "edit"].includes(toolName)) summary.writes += 1;
      else if (["search_content", "grep", "search_files", "glob"].includes(toolName)) summary.searches += 1;
      else summary.tools += 1;
      return summary;
    },
    { reads: 0, writes: 0, searches: 0, checks: 0, commands: 0, tools: 0 },
  );

  const items: string[] = [];

  if (categoryCounts.reads) items.push(`查看 ${categoryCounts.reads} 个文件`);
  if (categoryCounts.writes) items.push(`修改 ${categoryCounts.writes} 个文件`);
  if (categoryCounts.searches) items.push(`搜索 ${categoryCounts.searches} 次`);
  if (categoryCounts.checks) items.push(`运行 ${categoryCounts.checks} 次检查`);
  if (categoryCounts.commands) items.push(`运行 ${categoryCounts.commands} 个命令`);
  if (categoryCounts.tools) items.push(`调用 ${categoryCounts.tools} 个工具`);

  // Per-group tool-count visibility: total + failure annotation.
  const total = counts.totalTools + counts.shellCommands + counts.shellChecks;
  if (total > 0) {
    const failedTotal = counts.failedTools + counts.failedShells;
    if (failedTotal > 0) {
      items.push(`失败 ${failedTotal}/${total}`);
    }
  }

  if (counts.topTool) {
    items.push(`最多 ${counts.topTool}`);
  }

  return items;
}
