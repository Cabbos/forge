import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";
import type { PermissionRuleView, SetPermissionRuleInput } from "./types.ts";

export async function listPermissionRules(): Promise<PermissionRuleView[]> {
  if (!hasTauriRuntime()) return [];
  return invoke<PermissionRuleView[]>("list_permission_rules");
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

export async function resetPermissionRule(toolName: string): Promise<PermissionRuleView[]> {
  if (!hasTauriRuntime()) {
    throw new Error("Permission rule reset is not available outside Tauri runtime");
  }
  return invoke<PermissionRuleView[]>("reset_permission_rule", { toolName });
}
