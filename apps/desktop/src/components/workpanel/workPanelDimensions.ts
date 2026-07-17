export const DEFAULT_WORK_PANEL_WIDTH_PERCENT = 40;
export const MIN_WORK_PANEL_WIDTH_PERCENT = 34;
export const MAX_WORK_PANEL_WIDTH_PERCENT = 62;
export const MIN_WORK_PANEL_WIDTH_PX = 360;
export const MAX_WORK_PANEL_WIDTH_PX = 920;

export type WorkPanelViewportMode = "split" | "fixed" | "overlay";

export function normalizeWorkPanelWidthPercent(widthPercent: number | undefined): number {
  if (typeof widthPercent !== "number" || !Number.isFinite(widthPercent)) return DEFAULT_WORK_PANEL_WIDTH_PERCENT;
  return Math.min(MAX_WORK_PANEL_WIDTH_PERCENT, Math.max(MIN_WORK_PANEL_WIDTH_PERCENT, widthPercent));
}

export function getWorkPanelViewportMode(viewportWidth: number): WorkPanelViewportMode {
  if (viewportWidth < 720) return "overlay";
  if (viewportWidth < 900) return "fixed";
  return "split";
}

export function getWorkbenchWidth(viewportWidth: number): number {
  return Math.max(MIN_WORK_PANEL_WIDTH_PX, viewportWidth - 284);
}

export function getWorkPanelBounds(workbenchWidth: number): { min: number; max: number } {
  const safeWidth = Math.max(1, workbenchWidth);
  const min = Math.max(MIN_WORK_PANEL_WIDTH_PERCENT, Math.min(MAX_WORK_PANEL_WIDTH_PERCENT, (MIN_WORK_PANEL_WIDTH_PX / safeWidth) * 100));
  const max = Math.max(min, Math.min(MAX_WORK_PANEL_WIDTH_PERCENT, (MAX_WORK_PANEL_WIDTH_PX / safeWidth) * 100));
  return { min: round(min), max: round(max) };
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
