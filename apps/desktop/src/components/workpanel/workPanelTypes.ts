export type WorkPanelLauncherAction = "review" | "terminal" | "preview" | "files" | "subtasks";

export type PreviewTarget =
  | { type: "url"; url: string }
  | { type: "file"; path: string };

interface WorkPanelTabBase {
  id: string;
  label: string;
}

export type WorkPanelTab =
  | (WorkPanelTabBase & { kind: "review"; taskId: string })
  | (WorkPanelTabBase & { kind: "terminal"; taskId: string })
  | (WorkPanelTabBase & { kind: "preview"; target: PreviewTarget })
  | (WorkPanelTabBase & { kind: "file"; path: string })
  | (WorkPanelTabBase & { kind: "subtask"; taskId: string; subtaskId: string });

export interface WorkPanelTaskState {
  tabs: WorkPanelTab[];
  activeTabId: string | null;
  launcherOpen: boolean;
  widthPercent: number;
}
