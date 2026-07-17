import {
  AlertTriangle,
  CheckCircle,
  Loader2,
  RefreshCw,
  Wrench,
  XCircle,
} from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import type { DiagnosticCheck, DiagnosticsReport } from "@/lib/tauri";
import { buildDiagnosticRepairAction } from "./diagnosticsRepairView";
import { STATUS_CLASS, STATUS_ICON, STATUS_LABEL } from "./diagnosticsStatus";
import { formatDiagnosticDetail } from "./diagnosticsFormatters";

/** Report header, check rows, and loading/empty/error states for the diagnostics panel. */

export function DiagnosticsContent({
  report,
  isRefreshing,
  onRefresh,
}: {
  report: DiagnosticsReport;
  isRefreshing: boolean;
  onRefresh: () => void;
}) {
  const passCount = report.checks.filter((c) => c.status === "pass").length;
  const warnCount = report.checks.filter((c) => c.status === "warn").length;
  const failCount = report.checks.filter((c) => c.status === "fail").length;
  const overallStatus = report.ok ? "ok" : failCount > 0 ? "failures" : "warnings";
  const generatedAt = new Date(report.generatedAtMs).toLocaleString();
  const StatusIcon = report.ok ? CheckCircle : failCount > 0 ? XCircle : AlertTriangle;

  return (
    <>
      <div className="forge-settings-readonly-heading">
        <h4>
          <span className={`inline-flex items-center gap-1.5 ${STATUS_CLASS[report.ok ? "pass" : failCount > 0 ? "fail" : "warn"]}`}>
            <StatusIcon className="size-4" />
            {overallStatus === "ok" ? "系统正常" : overallStatus === "failures" ? "发现问题" : "有警告"}
          </span>
        </h4>
        <p>
          {generatedAt} · {passCount} 通过 · {warnCount} 警告 · {failCount} 失败
        </p>
      </div>
      <div className="flex items-center gap-2">
        <ButtonPrimitive
          type="button"
          onClick={onRefresh}
          disabled={isRefreshing}
          className="forge-settings-nav-button"
          aria-label="Refresh diagnostics"
        >
          <span className="forge-settings-nav-icon" aria-hidden="true">
            <RefreshCw className={`size-3.5 ${isRefreshing ? "animate-spin" : ""}`} />
          </span>
          <span className="forge-settings-nav-copy">
            <span className="forge-settings-nav-title">刷新</span>
          </span>
        </ButtonPrimitive>
      </div>
    </>
  );
}

export function DiagnosticsCheckRow({
  check,
  onRepair,
  repairingActionId,
}: {
  check: DiagnosticCheck;
  onRepair: (actionId: string) => void;
  repairingActionId: string | null;
}) {
  const Icon = STATUS_ICON[check.status] ?? CheckCircle;
  const cls = STATUS_CLASS[check.status] ?? STATUS_CLASS.pass;
  const repairAction = buildDiagnosticRepairAction(check);
  const isRepairing = repairAction != null && repairingActionId === repairAction.actionId;

  return (
    <div className="forge-settings-info-row">
      <dt>
        <span className={`inline-flex items-center gap-1 ${cls}`}>
          <Icon className="size-3" />
          {STATUS_LABEL[check.status] ?? check.status}
        </span>
      </dt>
      <dd>
        <div className="flex flex-col gap-0.5 min-w-0">
          <span className="text-xs font-medium truncate">{check.label}</span>
          <span className="text-[10px] font-mono text-muted-foreground">{check.id}</span>
          <span className="text-xs text-muted-foreground truncate">{check.message}</span>
          {check.remediation && (
            <span className="text-[10px] text-muted-foreground leading-tight">
              {check.remediation}
            </span>
          )}
          {check.detail != null && (
            <span className="text-[10px] font-mono text-muted-foreground leading-tight">
              {formatDiagnosticDetail(check.detail)}
            </span>
          )}
          {repairAction && (
            <ButtonPrimitive
              type="button"
              className="forge-diagnostic-repair-btn"
              onClick={() => onRepair(repairAction.actionId)}
              disabled={repairingActionId != null}
              aria-label={`${repairAction.label}: ${check.label}`}
            >
              {isRepairing ? (
                <Loader2 className="size-3 animate-spin" />
              ) : (
                <Wrench className="size-3" />
              )}
              <span>{repairAction.label}</span>
            </ButtonPrimitive>
          )}
        </div>
      </dd>
    </div>
  );
}

export function DiagnosticsLoading() {
  return (
    <div className="forge-settings-readonly-heading">
      <h4>加载诊断信息…</h4>
      <p>正在运行系统健康检查。</p>
    </div>
  );
}

export function DiagnosticsEmpty({ onRefresh }: { onRefresh: () => void }) {
  return (
    <div className="forge-settings-readonly-heading">
      <h4>暂无诊断数据</h4>
      <p>无法获取诊断报告。</p>
      <ButtonPrimitive type="button" onClick={onRefresh} className="forge-settings-nav-button mt-2">
        <span className="forge-settings-nav-icon" aria-hidden="true">
          <RefreshCw className="size-3.5" />
        </span>
        <span className="forge-settings-nav-copy">
          <span className="forge-settings-nav-title">重试</span>
        </span>
      </ButtonPrimitive>
    </div>
  );
}

export function DiagnosticsError({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="forge-settings-error" role="alert">
      <XCircle className="size-3.5" />
      <span>{message}</span>
      <ButtonPrimitive type="button" onClick={onRetry} className="ml-auto text-xs underline">
        重试
      </ButtonPrimitive>
    </div>
  );
}
