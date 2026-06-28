import type { BlockState } from "./protocol.ts";
import { parseWriteBoundary } from "./write-boundary.ts";

export interface PermissionConfirmTakeoverOptions {
  allowAnyOperation: boolean;
}

export function findLatestPendingWorkspaceConfirm(
  blocks: BlockState[],
  workingDir: string,
  options: PermissionConfirmTakeoverOptions,
): BlockState | null {
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block.event_type !== "confirm_ask") continue;
    if (block.metadata.confirmed === true || block.metadata.confirm_interrupted === true) continue;

    const boundary = parseWriteBoundary(block.metadata.boundary);
    if (!boundary) continue;
    if (normalizeProjectPath(boundary.workspacePath) !== normalizedWorkingDir) continue;
    if (!options.allowAnyOperation && !isWriteBoundaryOperation(boundary.operationLabel)) continue;
    if (!isAutoApprovableBoundary(block.metadata.boundary, workingDir, options.allowAnyOperation)) continue;
    return block;
  }
  return null;
}

function isWriteBoundaryOperation(operationLabel: string): boolean {
  return operationLabel === "写入文件" || operationLabel === "编辑文件" || operationLabel === "修改文件";
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}

function isAutoApprovableBoundary(
  boundary: unknown,
  workingDir: string,
  allowSensitiveWorkspaceFiles: boolean,
): boolean {
  if (!boundary || typeof boundary !== "object" || Array.isArray(boundary)) return false;
  const rawFiles = (boundary as { affected_files?: unknown }).affected_files;
  if (!Array.isArray(rawFiles)) return true;
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  return rawFiles.every((file) => {
    if (typeof file !== "string") return false;
    const normalizedFile = normalizeProjectPath(file);
    const projectRelativeFile = normalizedFile.startsWith(`${normalizedWorkingDir}/`)
      ? normalizedFile.slice(normalizedWorkingDir.length + 1)
      : normalizedFile;
    if (normalizedFile.startsWith("~")) return false;
    if (normalizedFile.startsWith("/") && normalizedFile !== normalizedWorkingDir && !normalizedFile.startsWith(`${normalizedWorkingDir}/`)) return false;
    if (projectRelativeFile === ".." || projectRelativeFile.startsWith("../") || projectRelativeFile.includes("/../")) return false;
    if (!allowSensitiveWorkspaceFiles && isSensitiveProjectPath(projectRelativeFile)) return false;
    return true;
  });
}

function isSensitiveProjectPath(path: string): boolean {
  const normalized = path.replace(/\\/g, "/").toLowerCase();
  return normalized === ".env" || normalized.startsWith(".env.") || normalized.endsWith("/.env") || normalized.includes("/.env.");
}
