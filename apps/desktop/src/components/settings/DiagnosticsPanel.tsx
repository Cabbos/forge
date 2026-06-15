import { useState, type FormEvent } from "react";
import { AlertTriangle, CheckCircle, Loader2, RefreshCw, Send, Wrench, XCircle } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useDiagnosticsReportQuery } from "@/hooks/queries/useDiagnosticsReportQuery";
import { useGatewayRuntimeStatusQuery } from "@/hooks/queries/useGatewayRuntimeStatusQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import type {
  DiagnosticCheck,
  DiagnosticsReport,
  GatewayRuntimeStatus,
  GatewayTriggerRunRecord,
} from "@/lib/tauri";
import { enqueueGatewayTrigger, runRepairAction } from "@/lib/tauri";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { buildGatewayRuntimeSummary, buildGatewayTriggerInput } from "./diagnosticsRuntimeView";
import { buildDiagnosticRepairAction } from "./diagnosticsRepairView";
import { formatMutationError } from "./settingsUtils";

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
  const queryClient = useQueryClient();
  const [repairingActionId, setRepairingActionId] = useState<string | null>(null);
  const [repairMessage, setRepairMessage] = useState<string | null>(null);
  const [repairError, setRepairError] = useState<string | null>(null);
  const {
    data: report,
    isLoading,
    isError,
    error,
    refetch,
    isFetching,
  } = useDiagnosticsReportQuery();
  const {
    data: runtimeStatus,
    isLoading: isRuntimeLoading,
    isError: isRuntimeError,
    error: runtimeError,
    refetch: refetchRuntime,
    isFetching: isRuntimeFetching,
  } = useGatewayRuntimeStatusQuery();

  const queryError = getQueryErrorMessage(isError ? error : null);
  const runtimeQueryError = getQueryErrorMessage(isRuntimeError ? runtimeError : null);
  const refreshAll = () => {
    void refetch();
    void refetchRuntime();
  };
  const handleRepair = async (actionId: string) => {
    if (repairingActionId) return;
    setRepairingActionId(actionId);
    setRepairMessage(null);
    setRepairError(null);
    try {
      const result = await runRepairAction(actionId);
      if (result.success) {
        setRepairMessage(result.message);
      } else {
        setRepairError(result.message);
      }
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.diagnosticsReport }),
        queryClient.invalidateQueries({ queryKey: queryKeys.gatewayRuntimeStatus }),
        refetch(),
        refetchRuntime(),
      ]);
    } catch (err) {
      setRepairError(formatMutationError(err));
    } finally {
      setRepairingActionId(null);
    }
  };

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
            isRefreshing={isFetching || isRuntimeFetching}
            onRefresh={refreshAll}
          />
        ) : (
          <DiagnosticsEmpty onRefresh={refreshAll} />
        )}
      </div>

      <GatewayRuntimePanel
        status={runtimeStatus}
        isLoading={isRuntimeLoading}
        isRefreshing={isRuntimeFetching}
        queryError={runtimeQueryError}
        onRefresh={() => refetchRuntime()}
      />

      {(repairError || repairMessage) && (
        <div
          className={repairError ? "forge-settings-error" : "forge-settings-success"}
          role={repairError ? "alert" : "status"}
        >
          {repairError ? <XCircle className="size-3.5" /> : <CheckCircle className="size-3.5" />}
          <span>{repairError ?? repairMessage}</span>
        </div>
      )}

      {/* ── Check list ── */}
      {report && report.checks.length > 0 && (
        <div className="forge-settings-readonly-panel">
          <div className="forge-settings-readonly-heading">
            <h4>检查清单</h4>
          </div>
          <div className="forge-settings-info-list">
            {report.checks.map((check) => (
              <DiagnosticsCheckRow
                key={check.id}
                check={check}
                onRepair={handleRepair}
                repairingActionId={repairingActionId}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function GatewayRuntimePanel({
  status,
  isLoading,
  isRefreshing,
  queryError,
  onRefresh,
}: {
  status?: GatewayRuntimeStatus;
  isLoading: boolean;
  isRefreshing: boolean;
  queryError: string | null;
  onRefresh: () => void;
}) {
  const [triggerMessage, setTriggerMessage] = useState("");
  const [triggerProfileId, setTriggerProfileId] = useState("");
  const [triggerProvider, setTriggerProvider] = useState("");
  const [triggerModel, setTriggerModel] = useState("");
  const [triggerWorkspacePath, setTriggerWorkspacePath] = useState("");
  const [isEnqueuingTrigger, setIsEnqueuingTrigger] = useState(false);
  const [triggerError, setTriggerError] = useState<string | null>(null);
  const [triggerMessageStatus, setTriggerMessageStatus] = useState<string | null>(null);

  if (isLoading && !status) {
    return (
      <div className="forge-settings-readonly-panel">
        <div className="forge-settings-readonly-heading">
          <h4>后台运行时</h4>
          <p>正在读取 gateway runtime 状态。</p>
        </div>
      </div>
    );
  }

  if (queryError && !status) {
    return (
      <div className="forge-settings-readonly-panel">
        <DiagnosticsError message={queryError} onRetry={onRefresh} />
      </div>
    );
  }

  const runtime = status ?? {
    ok: false,
    message: "暂无 gateway runtime 状态。",
    uptime_seconds: 0,
    active_sessions: 0,
    pending_triggers: 0,
    claimed_triggers: 0,
    dead_letter_runs: 0,
    recent_runs: [],
  };
  const summary = buildGatewayRuntimeSummary(runtime);
  const Icon = STATUS_ICON[summary.tone] ?? CheckCircle;
  const cls = STATUS_CLASS[summary.tone] ?? STATUS_CLASS.pass;
  const handleEnqueueTrigger = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (isEnqueuingTrigger) return;

    const built = buildGatewayTriggerInput({
      message: triggerMessage,
      profileId: triggerProfileId,
      provider: triggerProvider,
      model: triggerModel,
      workspacePath: triggerWorkspacePath,
    });
    if (!built.input) {
      setTriggerError(built.error);
      setTriggerMessageStatus(null);
      return;
    }

    setIsEnqueuingTrigger(true);
    setTriggerError(null);
    setTriggerMessageStatus(null);
    try {
      const result = await enqueueGatewayTrigger(built.input);
      setTriggerMessage("");
      setTriggerMessageStatus(
        `已排入 ${result.trigger_id}，当前 pending ${result.pending_triggers} 个。`,
      );
      onRefresh();
    } catch (error) {
      setTriggerError(formatMutationError(error));
    } finally {
      setIsEnqueuingTrigger(false);
    }
  };

  return (
    <div className="forge-settings-readonly-panel">
      <div className="forge-settings-readonly-heading">
        <h4>
          <span className={`inline-flex items-center gap-1.5 ${cls}`}>
            <Icon className="size-4" />
            后台运行时 · {summary.statusText}
          </span>
        </h4>
        <p>{runtime.message}</p>
      </div>
      <div className="forge-settings-info-list">
        <div className="forge-settings-info-row">
          <dt>队列</dt>
          <dd>{summary.counts}</dd>
        </div>
        <div className="forge-settings-info-row">
          <dt>会话</dt>
          <dd>
            {runtime.active_sessions} active · uptime {formatDuration(runtime.uptime_seconds)}
          </dd>
        </div>
        <div className="forge-settings-info-row">
          <dt>最近运行</dt>
          <dd>
            <GatewayRuntimeRuns runs={runtime.recent_runs} />
          </dd>
        </div>
      </div>
      <form className="forge-gateway-trigger-form" onSubmit={handleEnqueueTrigger}>
        <textarea
          className="forge-gateway-trigger-message"
          value={triggerMessage}
          onChange={(event) => setTriggerMessage(event.target.value)}
          placeholder="排入后台 trigger 的消息"
          rows={3}
          disabled={isEnqueuingTrigger}
        />
        <div className="forge-gateway-trigger-grid">
          <input
            className="forge-gateway-trigger-input"
            value={triggerProfileId}
            onChange={(event) => setTriggerProfileId(event.target.value)}
            placeholder="profile id"
            disabled={isEnqueuingTrigger}
          />
          <input
            className="forge-gateway-trigger-input"
            value={triggerProvider}
            onChange={(event) => setTriggerProvider(event.target.value)}
            placeholder="provider"
            disabled={isEnqueuingTrigger}
          />
          <input
            className="forge-gateway-trigger-input"
            value={triggerModel}
            onChange={(event) => setTriggerModel(event.target.value)}
            placeholder="model"
            disabled={isEnqueuingTrigger}
          />
          <input
            className="forge-gateway-trigger-input"
            value={triggerWorkspacePath}
            onChange={(event) => setTriggerWorkspacePath(event.target.value)}
            placeholder="workspace path"
            disabled={isEnqueuingTrigger}
          />
        </div>
        {triggerError && (
          <div className="forge-settings-error" role="alert">
            <XCircle className="size-3.5" />
            <span>{triggerError}</span>
          </div>
        )}
        {triggerMessageStatus && (
          <div className="forge-settings-success" role="status">
            <CheckCircle className="size-3.5" />
            <span>{triggerMessageStatus}</span>
          </div>
        )}
        <div className="flex items-center gap-2">
          <ButtonPrimitive
            type="submit"
            disabled={isEnqueuingTrigger}
            className="forge-settings-nav-button"
            aria-label="Enqueue gateway trigger"
          >
            <span className="forge-settings-nav-icon" aria-hidden="true">
              {isEnqueuingTrigger ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Send className="size-3.5" />
              )}
            </span>
            <span className="forge-settings-nav-copy">
              <span className="forge-settings-nav-title">排入 Trigger</span>
            </span>
          </ButtonPrimitive>
        </div>
      </form>
      <div className="flex items-center gap-2">
        <ButtonPrimitive
          type="button"
          onClick={onRefresh}
          disabled={isRefreshing}
          className="forge-settings-nav-button"
          aria-label="Refresh gateway runtime status"
        >
          <span className="forge-settings-nav-icon" aria-hidden="true">
            <RefreshCw className={`size-3.5 ${isRefreshing ? "animate-spin" : ""}`} />
          </span>
          <span className="forge-settings-nav-copy">
            <span className="forge-settings-nav-title">刷新运行时</span>
          </span>
        </ButtonPrimitive>
      </div>
    </div>
  );
}

function GatewayRuntimeRuns({ runs }: { runs: GatewayTriggerRunRecord[] }) {
  if (runs.length === 0) {
    return <span className="text-xs text-muted-foreground">暂无运行记录</span>;
  }

  return (
    <div className="flex min-w-0 flex-col gap-1">
      {runs.slice(0, 3).map((run) => (
        <span key={run.id} className="truncate text-xs text-muted-foreground">
          {run.status} · attempt {run.attempt} · {run.message}
        </span>
      ))}
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

function DiagnosticsCheckRow({
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

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h`;
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
      <ButtonPrimitive type="button" onClick={onRetry} className="ml-auto text-xs underline">
        重试
      </ButtonPrimitive>
    </div>
  );
}
