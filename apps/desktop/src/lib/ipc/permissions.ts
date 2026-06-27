import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";
import type {
  PermissionModeState,
  PermissionRuleView,
  SetPermissionModeInput,
  SetPermissionRuleInput,
} from "./types.ts";

export async function listPermissionRules(): Promise<PermissionRuleView[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<PermissionRuleView[]>("list_permission_rules");
}

export async function getPermissionMode(
  sessionId: string,
  workspacePath?: string | null,
): Promise<PermissionModeState> {
  if (!hasTauriRuntime()) {
    return {
      mode: "manual_confirm",
      workspace_path: null,
      session_scoped: true,
    };
  }
  return invoke<PermissionModeState>("get_permission_mode", {
    sessionId,
    workspacePath: workspacePath ?? null,
  });
}

export async function setPermissionRule(
  input: SetPermissionRuleInput,
): Promise<PermissionRuleView[]> {
  if (!hasTauriRuntime()) {
    throw new Error("Permission rule mutation is not available outside Tauri runtime");
  }
  return invoke<PermissionRuleView[]>("set_permission_rule", {
    toolName: input.toolName,
    decision: input.decision,
  });
}

export async function setPermissionMode(
  input: SetPermissionModeInput,
): Promise<PermissionModeState> {
  if (!hasTauriRuntime()) {
    throw new Error("Permission mode mutation is not available outside Tauri runtime");
  }
  return invoke<PermissionModeState>("set_permission_mode", {
    sessionId: input.sessionId,
    mode: input.mode,
    workspacePath: input.workspacePath ?? null,
  });
}

export async function resetPermissionRule(toolName: string): Promise<PermissionRuleView[]> {
  if (!hasTauriRuntime()) {
    throw new Error("Permission rule reset is not available outside Tauri runtime");
  }
  return invoke<PermissionRuleView[]>("reset_permission_rule", { toolName });
}
