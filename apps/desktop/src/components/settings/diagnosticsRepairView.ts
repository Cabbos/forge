import type { DiagnosticCheck, RepairResult } from "@/lib/tauri";

export interface DiagnosticRepairActionView {
  actionId: string;
  label: string;
}

const REPAIR_ACTION_LABELS: Record<string, string> = {
  restart_gateway: "重启 Gateway",
  reinstall_service: "重新安装服务",
  clear_snapshot_cache: "清除快照缓存",
  clear_logs: "清除日志",
  check_config: "检查配置",
};

export function buildDiagnosticRepairAction(
  check: DiagnosticCheck,
): DiagnosticRepairActionView | null {
  if (check.status === "pass" || !check.repairActionId) {
    return null;
  }

  return {
    actionId: check.repairActionId,
    label: REPAIR_ACTION_LABELS[check.repairActionId] ?? "运行修复",
  };
}

export function formatRepairResultMessage(result: RepairResult): string {
  if (!result.verification) {
    return result.message;
  }

  const status = result.verification.ok ? "验证通过" : "验证失败";
  return `${result.message} ${result.verification.label}: ${status} - ${result.verification.message}`;
}
