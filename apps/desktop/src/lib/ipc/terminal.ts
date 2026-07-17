import { invoke } from "@tauri-apps/api/core";

export interface WorkspaceTerminalInfo {
  terminal_id: string;
  task_id: string;
  working_dir: string;
}

export interface WorkspaceTerminalOutput {
  terminal_id: string;
  task_id: string;
  chunk: string;
  exited: boolean;
}

export function startWorkspaceTerminal(input: {
  taskId: string;
  sessionId?: string | null;
  workingDir?: string | null;
  rows: number;
  cols: number;
}): Promise<WorkspaceTerminalInfo> {
  return invoke<WorkspaceTerminalInfo>("start_workspace_terminal", {
    taskId: input.taskId,
    sessionId: input.sessionId ?? null,
    workingDir: input.workingDir ?? null,
    rows: input.rows,
    cols: input.cols,
  });
}

export function writeWorkspaceTerminal(
  taskId: string,
  terminalId: string,
  data: string,
): Promise<void> {
  return invoke("write_workspace_terminal", { taskId, terminalId, data });
}

export function resizeWorkspaceTerminal(
  taskId: string,
  terminalId: string,
  rows: number,
  cols: number,
): Promise<void> {
  return invoke("resize_workspace_terminal", { taskId, terminalId, rows, cols });
}

export function closeWorkspaceTerminal(taskId: string, terminalId: string): Promise<void> {
  return invoke("close_workspace_terminal", { taskId, terminalId });
}
