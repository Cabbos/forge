import type { BlockState } from "@/lib/protocol";

export type ProcessActivityState = "error" | "running" | "done";

export function deriveToolActivityView(blocks: BlockState[]) {
  const hasError = blocks.some(isProcessBlockError);
  const isRunning = blocks.some((block) => !block.isComplete);
  const state: ProcessActivityState = hasError ? "error" : isRunning ? "running" : "done";

  return {
    hasError,
    isRunning,
    label: processActivityLabel(state, blocks.length),
    state,
    summaryItems: summarizeActivity(blocks),
    tone: processActivityTone(state),
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

function summarizeActivity(blocks: BlockState[]) {
  const counts = blocks.reduce(
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

  return [
    counts.reads ? `查看 ${counts.reads} 个文件` : "",
    counts.writes ? `修改 ${counts.writes} 个文件` : "",
    counts.searches ? `搜索 ${counts.searches} 次` : "",
    counts.checks ? `运行 ${counts.checks} 次检查` : "",
    counts.commands ? `运行 ${counts.commands} 个命令` : "",
    counts.tools ? `调用 ${counts.tools} 个工具` : "",
  ].filter(Boolean);
}
