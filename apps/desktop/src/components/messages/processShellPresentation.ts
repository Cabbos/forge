import type { BlockState } from "@/lib/protocol";

export function deriveShellView(block: BlockState) {
  const exitCode = block.metadata.exit_code as number | undefined;
  const isError = exitCode !== undefined && exitCode !== 0;
  const isRunning = !block.isComplete;
  const output = block.content || "";

  return {
    command: (block.metadata.command as string) || "命令",
    exitCode,
    isError,
    isRunning,
    output,
    outputSections: parseShellOutput(output, isError),
    state: isRunning ? "running" : isError ? "error" : "done",
    tone: isError ? "error" : "default",
  };
}

function parseShellOutput(output: string, isError: boolean) {
  const lines = output.split("\n");
  const sections: Array<{ label: string; content: string }> = [];
  let currentLabel: string | null = null;
  let currentLines: string[] = [];

  const flush = () => {
    if (!currentLabel) return;
    sections.push({ label: currentLabel, content: currentLines.join("\n").trimEnd() });
    currentLines = [];
  };

  for (const line of lines) {
    const match = line.match(/^(stdout|stderr):\s*$/i);
    if (match) {
      flush();
      currentLabel = match[1].toLowerCase();
      continue;
    }
    if (!currentLabel) currentLabel = isError ? "output" : "stdout";
    currentLines.push(line);
  }
  flush();

  return sections.length ? sections : [{ label: isError ? "output" : "stdout", content: output }];
}
