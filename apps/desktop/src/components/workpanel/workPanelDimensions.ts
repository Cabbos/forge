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
  const safeViewportWidth = Number.isFinite(viewportWidth) ? viewportWidth : 0;
  if (safeViewportWidth < 720) return "overlay";
  if (safeViewportWidth < 900) return "fixed";
  return "split";
}

export function getWorkbenchWidth(viewportWidth: number): number {
  const safeViewportWidth = Number.isFinite(viewportWidth) ? viewportWidth : 0;
  return Math.max(MIN_WORK_PANEL_WIDTH_PX, safeViewportWidth - 284);
}

export function getWorkPanelBounds(workbenchWidth: number): { min: number; max: number } {
  const safeWidth = Number.isFinite(workbenchWidth) ? Math.max(1, workbenchWidth) : 1;
  const min = Math.max(MIN_WORK_PANEL_WIDTH_PERCENT, Math.min(MAX_WORK_PANEL_WIDTH_PERCENT, (MIN_WORK_PANEL_WIDTH_PX / safeWidth) * 100));
  const max = Math.max(min, Math.min(MAX_WORK_PANEL_WIDTH_PERCENT, (MAX_WORK_PANEL_WIDTH_PX / safeWidth) * 100));
  return { min: round(min), max: round(max) };
}

export function clampWorkPanelWidthPercent(widthPercent: number, bounds: { min: number; max: number }): number {
  return Math.min(bounds.max, Math.max(bounds.min, normalizeWorkPanelWidthPercent(widthPercent)));
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}
