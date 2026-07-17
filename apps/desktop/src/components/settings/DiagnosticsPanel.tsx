import { useState } from "react";
import { CheckCircle, XCircle } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useDiagnosticsReportQuery } from "@/hooks/queries/useDiagnosticsReportQuery";
import { useGatewayRuntimeStatusQuery } from "@/hooks/queries/useGatewayRuntimeStatusQuery";
import { useGatewaySessionsQuery } from "@/hooks/queries/useGatewaySessionsQuery";
import { useGatewayTriggersQuery } from "@/hooks/queries/useGatewayTriggersQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { runRepairAction } from "@/lib/tauri";
import { GatewayRuntimePanel } from "./GatewayRuntimePanel";
import {
  DiagnosticsCheckRow,
  DiagnosticsContent,
  DiagnosticsEmpty,
  DiagnosticsError,
  DiagnosticsLoading,
} from "./DiagnosticsReportSections";
import { formatRepairResultMessage } from "./diagnosticsRepairView";
import { formatMutationError } from "./settingsUtils";

/** Orchestrates diagnostics queries, repair actions, and the gateway runtime panel. */
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
