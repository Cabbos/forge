import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "./types";
import { getRememberedWorkingDir } from "./app";

export async function getProjectRuntimeStatus(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectRuntimeStatus> {
  if (!hasTauriRuntime()) return fallbackProjectRuntimeStatus();
  return invoke("get_project_runtime_status", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function startProjectDevServer(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectRuntimeStatus> {
  return invoke("start_project_dev_server", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function stopProjectDevServer(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectRuntimeStatus> {
  return invoke("stop_project_dev_server", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function openProjectPreview(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectRuntimeStatus> {
  return invoke("open_project_preview", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function getProjectCheckpointStatus(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectCheckpointStatus> {
  if (!hasTauriRuntime()) return fallbackProjectCheckpointStatus();
  return invoke("get_project_checkpoint_status", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function createProjectCheckpoint(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectCheckpointStatus> {
  return invoke("create_project_checkpoint", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function restoreProjectCheckpoint(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ProjectCheckpointStatus> {
  return invoke("restore_project_checkpoint", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

function fallbackProjectRuntimeStatus(): ProjectRuntimeStatus {
  return {
    working_dir: getRememberedWorkingDir() ?? "",
    has_package_json: false,
    package_manager: "npm",
    dev_script: null,
    command: null,
    port: 1420,
    url: "http://localhost:1420",
    running: false,
    managed: false,
    pid: null,
    can_start: false,
    can_stop: false,
    can_open: false,
    message: "在桌面应用中读取交付状态",
    logs: [],
  };
}

function fallbackProjectCheckpointStatus(): ProjectCheckpointStatus {
  return {
    working_dir: getRememberedWorkingDir() ?? "",
    is_git_repo: false,
    dirty: false,
    last_checkpoint: null,
    restorable: false,
    snapshot_warning: null,
    message: "在桌面应用中读取检查点",
  };
}
