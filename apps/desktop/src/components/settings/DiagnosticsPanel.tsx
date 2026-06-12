import { RefreshCw, CheckCircle, AlertTriangle, XCircle } from "lucide-react";
import { useDiagnosticsReportQuery } from "@/hooks/queries/useDiagnosticsReportQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import type { DiagnosticCheck, DiagnosticsReport } from "@/lib/tauri";

const STATUS_ICON: Record<string, typeof CheckCircle> = {
  pass: CheckCircle,
  warn: AlertTriangle,
  fail: XCircle,
};

const STATUS_CLASS: Record<string, string> = {
  pass: "text-green-600",
  warn: "text-amber-500",
  fail: "text-red-500",
};

const STATUS_LABEL: Record<string, string> = {
  pass: "Pass",
  warn: "Warn",
  fail: "Fail",
};

export function DiagnosticsPanel() {
  const {
    data: report,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useDiagnosticsReportQuery();

  const queryError = getQueryErrorMessage(isError ? error : null);

  return (
    <div className="forge-settings-panel-stack">
      {/* ── Header with refresh ── */}
      <div className="forge-settings-readonly-panel">
        {!report && isLoading ? (
          <DiagnosticsLoading />
        ) : queryError ? (
          <DiagnosticsError message={queryError} onRetry={() => refetch()} />
        ) : report ? (
          <DiagnosticsContent
            report={report}
            isRefreshing={isFetching}
            onRefresh={() => refetch()}
          />
        ) : (
          <DiagnosticsEmpty onRefresh={() => refetch()} />
        )}
      </div>

      {/* ── Check list ── */}
      {report && report.checks.length > 0 && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-settings-readonly-heading">
            <h4>检查清单</h4>
          </div>
          <div className="forge-settings-info-list">
            {report.checks.map((check) => (
              <DiagnosticsCheckRow key={check.id} check={check} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function DiagnosticsContent({
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
        <button
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
        </button>
      </div>
    </>
  );
}

function DiagnosticsCheckRow({ check }: { check: DiagnosticCheck }) {
  const Icon = STATUS_ICON[check.status] ?? CheckCircle;
  const cls = STATUS_CLASS[check.status] ?? STATUS_CLASS.pass;

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
        </div>
      </dd>
    </div>
  );
}

function formatDiagnosticDetail(detail: unknown): string {
  if (detail === null || detail === undefined) return "";
  if (typeof detail !== "object") return String(detail);
  const entries = Object.entries(detail as Record<string, unknown>);
  if (entries.length === 0) return "{}";
  return entries
    .slice(0, 4)
    .map(([key, value]) => `${key}: ${formatDetailValue(value)}`)
    .join(" · ");
}

function formatDetailValue(value: unknown): string {
  if (Array.isArray(value)) return `[${value.length}]`;
  if (value && typeof value === "object") return "{...}";
  return String(value);
}

function DiagnosticsLoading() {
  return (
    <div className="forge-settings-readonly-heading">
      <h4>加载诊断信息…</h4>
      <p>正在运行系统健康检查。</p>
    </div>
  );
}

function DiagnosticsEmpty({ onRefresh }: { onRefresh: () => void }) {
  return (
    <div className="forge-settings-readonly-heading">
      <h4>暂无诊断数据</h4>
      <p>无法获取诊断报告。</p>
      <button type="button" onClick={onRefresh} className="forge-settings-nav-button mt-2">
        <span className="forge-settings-nav-icon" aria-hidden="true">
          <RefreshCw className="size-3.5" />
        </span>
        <span className="forge-settings-nav-copy">
          <span className="forge-settings-nav-title">重试</span>
        </span>
      </button>
    </div>
  );
}

function DiagnosticsError({
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
      <button type="button" onClick={onRetry} className="ml-auto text-xs underline">
        重试
      </button>
    </div>
  );
}
