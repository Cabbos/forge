import { useState, type FormEvent } from "react";
import {
  AlertTriangle,
  CheckCircle,
  Info,
  Loader2,
  RefreshCw,
  RotateCcw,
  Send,
  Wrench,
  XCircle,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useDiagnosticsReportQuery } from "@/hooks/queries/useDiagnosticsReportQuery";
import { useGatewayRuntimeStatusQuery } from "@/hooks/queries/useGatewayRuntimeStatusQuery";
import { useGatewaySessionsQuery } from "@/hooks/queries/useGatewaySessionsQuery";
import { useGatewayTriggersQuery } from "@/hooks/queries/useGatewayTriggersQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import type {
  DiagnosticCheck,
  DiagnosticsReport,
  GatewayPendingTrigger,
  GatewayRuntimeStatus,
  GatewaySessionInfo,
  GatewayTriggerRunRecord,
} from "@/lib/tauri";
import {
  cancelGatewayTrigger,
  enqueueGatewayTrigger,
  getGatewayTriggerRun,
  replayGatewayTriggerRun,
  runRepairAction,
} from "@/lib/tauri";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import {
  buildGatewayRuntimeSummary,
  buildGatewaySessionRows,
  buildGatewayTriggerInput,
  buildGatewayTriggerRunRows,
  buildGatewayTriggerRows,
} from "./diagnosticsRuntimeView";
import {
  buildDiagnosticRepairAction,
  formatRepairResultMessage,
} from "./diagnosticsRepairView";
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
  const {
    data: gatewayTriggers = [],
    isLoading: isGatewayTriggersLoading,
    isError: isGatewayTriggersError,
    error: gatewayTriggersError,
    refetch: refetchGatewayTriggers,
    isFetching: isGatewayTriggersFetching,
  } = useGatewayTriggersQuery();
  const {
    data: gatewaySessions = [],
    isLoading: isGatewaySessionsLoading,
    isError: isGatewaySessionsError,
    error: gatewaySessionsError,
    refetch: refetchGatewaySessions,
    isFetching: isGatewaySessionsFetching,
  } = useGatewaySessionsQuery();

  const queryError = getQueryErrorMessage(isError ? error : null);
  const runtimeQueryError = getQueryErrorMessage(isRuntimeError ? runtimeError : null);
  const gatewayTriggersQueryError = getQueryErrorMessage(
    isGatewayTriggersError ? gatewayTriggersError : null,
  );
  const gatewaySessionsQueryError = getQueryErrorMessage(
    isGatewaySessionsError ? gatewaySessionsError : null,
  );
  const refreshAll = () => {
    void refetch();
    void refetchRuntime();
    void refetchGatewayTriggers();
    void refetchGatewaySessions();
  };
  const handleRepair = async (actionId: string) => {
    if (repairingActionId) return;
    setRepairingActionId(actionId);
    setRepairMessage(null);
    setRepairError(null);
    try {
      const result = await runRepairAction(actionId);
      const formattedMessage = formatRepairResultMessage(result);
      if (result.success) {
        setRepairMessage(formattedMessage);
      } else {
        setRepairError(formattedMessage);
      }
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.diagnosticsReport }),
        queryClient.invalidateQueries({ queryKey: queryKeys.gatewayRuntimeStatus }),
        queryClient.invalidateQueries({ queryKey: queryKeys.gatewayTriggers }),
        queryClient.invalidateQueries({ queryKey: queryKeys.gatewaySessions }),
        refetch(),
        refetchRuntime(),
        refetchGatewayTriggers(),
        refetchGatewaySessions(),
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
        triggers={gatewayTriggers}
        sessions={gatewaySessions}
        isLoading={isRuntimeLoading}
        isTriggersLoading={isGatewayTriggersLoading}
        isSessionsLoading={isGatewaySessionsLoading}
        isRefreshing={isRuntimeFetching || isGatewayTriggersFetching || isGatewaySessionsFetching}
        queryError={runtimeQueryError}
        triggersQueryError={gatewayTriggersQueryError}
        sessionsQueryError={gatewaySessionsQueryError}
        onRefresh={() => {
          void refetchRuntime();
          void refetchGatewayTriggers();
          void refetchGatewaySessions();
        }}
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
  triggers,
  sessions,
  isLoading,
  isTriggersLoading,
  isSessionsLoading,
  isRefreshing,
  queryError,
  triggersQueryError,
  sessionsQueryError,
  onRefresh,
}: {
  status?: GatewayRuntimeStatus;
  triggers: GatewayPendingTrigger[];
  sessions: GatewaySessionInfo[];
  isLoading: boolean;
  isTriggersLoading: boolean;
  isSessionsLoading: boolean;
  isRefreshing: boolean;
  queryError: string | null;
  triggersQueryError: string | null;
  sessionsQueryError: string | null;
  onRefresh: () => void;
}) {
  const [triggerMessage, setTriggerMessage] = useState("");
  const [triggerProfileId, setTriggerProfileId] = useState("");
  const [triggerProvider, setTriggerProvider] = useState("");
  const [triggerModel, setTriggerModel] = useState("");
  const [triggerWorkspacePath, setTriggerWorkspacePath] = useState("");
  const [isEnqueuingTrigger, setIsEnqueuingTrigger] = useState(false);
  const [cancelingTriggerId, setCancelingTriggerId] = useState<string | null>(null);
  const [replayingRunId, setReplayingRunId] = useState<string | null>(null);
  const [inspectingRunId, setInspectingRunId] = useState<string | null>(null);
  const [selectedRunDetail, setSelectedRunDetail] = useState<GatewayTriggerRunRecord | null>(null);
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
    runtime_tasks: [],
  };
  const summary = buildGatewayRuntimeSummary(runtime);
  const triggerRows = buildGatewayTriggerRows(triggers);
  const sessionRows = buildGatewaySessionRows(sessions, Date.now());
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
  const handleCancelTrigger = async (triggerId: string) => {
    if (cancelingTriggerId) return;
    setCancelingTriggerId(triggerId);
    setTriggerError(null);
    setTriggerMessageStatus(null);
    try {
      const result = await cancelGatewayTrigger(triggerId);
      setTriggerMessageStatus(
        result.removed
          ? `已取消 ${result.trigger_id}，当前 pending ${result.pending_triggers} 个。`
          : `${result.trigger_id} 已不在队列中。`,
      );
      onRefresh();
    } catch (error) {
      setTriggerError(formatMutationError(error));
    } finally {
      setCancelingTriggerId(null);
    }
  };
  const handleReplayRun = async (runId: string) => {
    if (replayingRunId) return;
    setReplayingRunId(runId);
    setTriggerError(null);
    setTriggerMessageStatus(null);
    try {
      const result = await replayGatewayTriggerRun(runId);
      setTriggerMessageStatus(
        `已重放 ${result.run_id} 为 ${result.trigger_id}，当前 pending ${result.pending_triggers} 个。`,
      );
      onRefresh();
    } catch (error) {
      setTriggerError(formatMutationError(error));
    } finally {
      setReplayingRunId(null);
    }
  };
  const handleInspectRun = async (runId: string) => {
    if (inspectingRunId) return;
    if (selectedRunDetail?.id === runId) {
      setSelectedRunDetail(null);
      return;
    }
    setInspectingRunId(runId);
    setTriggerError(null);
    setTriggerMessageStatus(null);
    try {
      const result = await getGatewayTriggerRun(runId);
      setSelectedRunDetail(result.run);
    } catch (error) {
      setTriggerError(formatMutationError(error));
    } finally {
      setInspectingRunId(null);
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
          <dt>会话详情</dt>
          <dd>
            <GatewaySessionRegistry
              rows={sessionRows}
              isLoading={isSessionsLoading}
              queryError={sessionsQueryError}
            />
          </dd>
        </div>
        <div className="forge-settings-info-row">
          <dt>后台循环</dt>
          <dd>
            <GatewayRuntimeTasks tasks={runtime.runtime_tasks} />
          </dd>
        </div>
        <div className="forge-settings-info-row">
          <dt>最近运行</dt>
          <dd>
            <GatewayRuntimeRuns
              runs={runtime.recent_runs}
              replayingRunId={replayingRunId}
              inspectingRunId={inspectingRunId}
              selectedRunDetail={selectedRunDetail}
              onReplay={handleReplayRun}
              onInspect={handleInspectRun}
            />
          </dd>
        </div>
        <div className="forge-settings-info-row">
          <dt>队列详情</dt>
          <dd>
            <GatewayTriggerQueue
              rows={triggerRows}
              isLoading={isTriggersLoading}
              queryError={triggersQueryError}
              cancelingTriggerId={cancelingTriggerId}
              onCancel={handleCancelTrigger}
            />
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

function GatewaySessionRegistry({
  rows,
  isLoading,
  queryError,
}: {
  rows: ReturnType<typeof buildGatewaySessionRows>;
  isLoading: boolean;
  queryError: string | null;
}) {
  if (isLoading && rows.length === 0) {
    return <span className="text-xs text-muted-foreground">正在读取 session registry</span>;
  }

  if (queryError && rows.length === 0) {
    return <span className="text-xs text-red-500">{queryError}</span>;
  }

  if (rows.length === 0) {
    return <span className="text-xs text-muted-foreground">暂无 gateway session</span>;
  }

  return (
    <div className="forge-gateway-trigger-queue">
      {rows.slice(0, 5).map((row) => (
        <div key={row.id} className="forge-gateway-trigger-row">
          <div className="forge-gateway-trigger-row-main">
            <span className="forge-gateway-trigger-row-title">
              <span data-state={row.stateLabel}>{row.stateLabel}</span>
              <code>{row.id}</code>
            </span>
            <span className="forge-gateway-trigger-row-message">{row.runtime}</span>
            <span className="forge-gateway-trigger-row-meta">{row.subtitle}</span>
            {row.workspacePath && (
              <span className="forge-gateway-trigger-row-meta">{row.workspacePath}</span>
            )}
          </div>
        </div>
      ))}
      {rows.length > 5 && (
        <span className="text-[10px] text-muted-foreground">
          另有 {rows.length - 5} 个 session，可用 CLI 查看完整列表。
        </span>
      )}
    </div>
  );
}

function GatewayRuntimeTasks({
  tasks,
}: {
  tasks: GatewayRuntimeStatus["runtime_tasks"];
}) {
  if (tasks.length === 0) {
    return <span className="text-xs text-muted-foreground">暂无后台循环状态</span>;
  }

  return (
    <div className="flex flex-wrap gap-1.5">
      {tasks.map((task) => {
        const cls = task.running && !task.last_error ? STATUS_CLASS.pass : STATUS_CLASS.warn;
        return (
          <span
            key={task.name}
            className={`inline-flex max-w-full items-center gap-1 rounded border border-border px-1.5 py-0.5 text-[11px] ${cls}`}
            title={task.last_error ?? undefined}
          >
            <span>{formatRuntimeTaskName(task.name)}</span>
            <span>{task.running ? "running" : "stopped"}</span>
          </span>
        );
      })}
    </div>
  );
}

function formatRuntimeTaskName(name: string): string {
  switch (name) {
    case "webhook_listener":
      return "webhook";
    case "trigger_runner":
      return "trigger";
    case "scheduler_tick":
      return "scheduler";
    default:
      return name.replace(/_/g, " ");
  }
}

function GatewayRuntimeRuns({
  runs,
  replayingRunId,
  inspectingRunId,
  selectedRunDetail,
  onReplay,
  onInspect,
}: {
  runs: GatewayTriggerRunRecord[];
  replayingRunId: string | null;
  inspectingRunId: string | null;
  selectedRunDetail: GatewayTriggerRunRecord | null;
  onReplay: (runId: string) => void;
  onInspect: (runId: string) => void;
}) {
  if (runs.length === 0) {
    return <span className="text-xs text-muted-foreground">暂无运行记录</span>;
  }

  const rows = buildGatewayTriggerRunRows(runs).slice(0, 3);
  return (
    <div className="forge-gateway-trigger-queue">
      {rows.map((row) => (
        <div key={row.id} className="forge-gateway-trigger-row">
          <div className="forge-gateway-trigger-row-main">
            <span className="forge-gateway-trigger-row-title">
              <span data-state={row.canReplay ? "pending" : "claimed"}>
                {row.canReplay ? "replayable" : "legacy"}
              </span>
              <code>{row.id}</code>
            </span>
            <span className="forge-gateway-trigger-row-message">{row.title}</span>
            <span className="forge-gateway-trigger-row-meta">{row.message}</span>
            <span className="forge-gateway-trigger-row-meta">{row.subtitle}</span>
            {selectedRunDetail?.id === row.id && (
              <span className="forge-gateway-trigger-row-meta">
                {formatRunDetail(selectedRunDetail)}
              </span>
            )}
          </div>
          <ButtonPrimitive
            type="button"
            className="forge-gateway-trigger-cancel-btn"
            disabled={inspectingRunId != null}
            aria-label={`Inspect gateway trigger run ${row.id}`}
            title="查看 trigger run 详情"
            onClick={() => onInspect(row.id)}
          >
            {inspectingRunId === row.id ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Info className="size-3" />
            )}
          </ButtonPrimitive>
          <ButtonPrimitive
            type="button"
            className="forge-gateway-trigger-cancel-btn"
            disabled={!row.canReplay || replayingRunId != null}
            aria-label={`Replay gateway trigger run ${row.id}`}
            title={row.canReplay ? "重放 trigger run" : "旧记录缺少 trigger metadata"}
            onClick={() => onReplay(row.id)}
          >
            {replayingRunId === row.id ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <RotateCcw className="size-3" />
            )}
          </ButtonPrimitive>
        </div>
      ))}
    </div>
  );
}

function formatRunDetail(run: GatewayTriggerRunRecord): string {
  const parts = [
    `started=${run.started_at_ms}`,
    `ended=${run.ended_at_ms}`,
    `workspace=${run.workspace_path?.trim() || "-"}`,
    `trigger_message=${run.trigger_message?.trim() || "-"}`,
  ];
  return parts.join(" · ");
}

function GatewayTriggerQueue({
  rows,
  isLoading,
  queryError,
  cancelingTriggerId,
  onCancel,
}: {
  rows: ReturnType<typeof buildGatewayTriggerRows>;
  isLoading: boolean;
  queryError: string | null;
  cancelingTriggerId: string | null;
  onCancel: (triggerId: string) => void;
}) {
  if (isLoading && rows.length === 0) {
    return <span className="text-xs text-muted-foreground">正在读取 trigger 队列</span>;
  }

  if (queryError && rows.length === 0) {
    return <span className="text-xs text-red-500">{queryError}</span>;
  }

  if (rows.length === 0) {
    return <span className="text-xs text-muted-foreground">暂无 pending trigger</span>;
  }

  return (
    <div className="forge-gateway-trigger-queue">
      {rows.slice(0, 6).map((row) => {
        const isCanceling = cancelingTriggerId === row.id;
        return (
          <div key={row.id} className="forge-gateway-trigger-row">
            <div className="forge-gateway-trigger-row-main">
              <span className="forge-gateway-trigger-row-title">
                <span data-state={row.stateLabel}>{row.stateLabel}</span>
                <code>{row.id}</code>
              </span>
              <span className="forge-gateway-trigger-row-message">{row.message}</span>
              <span className="forge-gateway-trigger-row-meta">{row.subtitle}</span>
              {row.workspacePath && (
                <span className="forge-gateway-trigger-row-meta">{row.workspacePath}</span>
              )}
            </div>
            <ButtonPrimitive
              type="button"
              className="forge-gateway-trigger-cancel-btn"
              disabled={Boolean(cancelingTriggerId)}
              aria-label={`Cancel gateway trigger ${row.id}`}
              title="取消 trigger"
              onClick={() => onCancel(row.id)}
            >
              {isCanceling ? (
                <Loader2 className="size-3 animate-spin" />
              ) : (
                <XCircle className="size-3" />
              )}
            </ButtonPrimitive>
          </div>
        );
      })}
      {rows.length > 6 && (
        <span className="text-[10px] text-muted-foreground">
          另有 {rows.length - 6} 个 trigger，可用 CLI 查看完整列表。
        </span>
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
