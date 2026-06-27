import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, RotateCcw, ShieldAlert, XCircle } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import {
  getPermissionMode,
  listPermissionRules,
  resetPermissionRule,
  setPermissionMode,
  type PermissionModeState,
  setPermissionRule,
  type PermissionRuleDecision,
  type PermissionRuleView,
} from "@/lib/tauri";
import { useActiveSession, useActiveWorkspace } from "@/store";

const TOOL_LABELS: Record<string, string> = {
  read_file: "读取文件",
  write_to_file: "写入文件",
  edit_file: "编辑文件",
  run_shell: "Shell 命令",
  mcp_read_resource: "读取 MCP 资源",
  mcp_get_prompt: "读取 MCP Prompt",
};

const MANAGED_TOOLS = [
  "write_to_file",
  "edit_file",
  "run_shell",
  "mcp_read_resource",
  "mcp_get_prompt",
];

export function PermissionsPanel() {
  const activeSession = useActiveSession();
  const activeWorkspace = useActiveWorkspace();
  const activeSessionId = activeSession?.id ?? null;
  const activeWorkspacePath = activeWorkspace?.path ?? activeSession?.workingDir ?? null;
  const [rules, setRules] = useState<PermissionRuleView[]>([]);
  const [modeState, setModeState] = useState<PermissionModeState>(manualModeState);
  const [loading, setLoading] = useState(true);
  const [busyTool, setBusyTool] = useState<string | null>(null);
  const [modeBusy, setModeBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadRules = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setRules(await listPermissionRules());
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setLoading(false);
    }
  }, []);

  const loadMode = useCallback(async () => {
    if (!activeSessionId) {
      setModeState(manualModeState);
      return;
    }
    try {
      setModeState(await getPermissionMode(activeSessionId, activeWorkspacePath));
    } catch (event) {
      setError(formatPermissionError(event));
    }
  }, [activeSessionId, activeWorkspacePath]);

  useEffect(() => {
    void loadRules();
    void loadMode();
  }, [loadMode, loadRules]);

  const rows = useMemo(() => buildPermissionRows(rules), [rules]);
  const trustAvailable = Boolean(activeSessionId && activeWorkspacePath);
  const trustActive = modeState.mode === "trust_current_project";
  const fullAccessActive = modeState.mode === "full_access";
  const modeActive = trustActive || fullAccessActive;

  const updateRule = useCallback(async (toolName: string, decision: PermissionRuleDecision) => {
    setBusyTool(toolName);
    setError(null);
    try {
      setRules(await setPermissionRule({ toolName, decision }));
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setBusyTool(null);
    }
  }, []);

  const resetRule = useCallback(async (toolName: string) => {
    setBusyTool(toolName);
    setError(null);
    try {
      setRules(await resetPermissionRule(toolName));
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setBusyTool(null);
    }
  }, []);

  const trustCurrentProject = useCallback(async () => {
    if (!activeSessionId || !activeWorkspacePath) return;
    setModeBusy(true);
    setError(null);
    try {
      setModeState(await setPermissionMode({
        sessionId: activeSessionId,
        mode: "trust_current_project",
        workspacePath: activeWorkspacePath,
      }));
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setModeBusy(false);
    }
  }, [activeSessionId, activeWorkspacePath]);

  const fullAccessCurrentProject = useCallback(async () => {
    if (!activeSessionId || !activeWorkspacePath) return;
    setModeBusy(true);
    setError(null);
    try {
      setModeState(await setPermissionMode({
        sessionId: activeSessionId,
        mode: "full_access",
        workspacePath: activeWorkspacePath,
      }));
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setModeBusy(false);
    }
  }, [activeSessionId, activeWorkspacePath]);

  const restoreManualConfirm = useCallback(async () => {
    if (!activeSessionId) return;
    setModeBusy(true);
    setError(null);
    try {
      setModeState(await setPermissionMode({
        sessionId: activeSessionId,
        mode: "manual_confirm",
        workspacePath: activeWorkspacePath,
      }));
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setModeBusy(false);
    }
  }, [activeSessionId, activeWorkspacePath]);

  return (
    <section
      data-testid="settings-permissions-panel"
      className="forge-settings-permissions-panel"
    >
      <div className="forge-settings-permissions-toolbar">
        <div className="min-w-0">
          <h4 className="forge-settings-panel-title">权限规则</h4>
          <p className="forge-settings-panel-description">
            写入、Shell 和 MCP 操作会按这里的规则决定是否直接继续。
          </p>
        </div>
        <ForgeButton size="xs" variant="outline" onClick={loadRules} disabled={loading}>
          <RotateCcw className="size-3" />
          刷新
        </ForgeButton>
      </div>

      {error && (
        <div className="forge-settings-error" role="alert">
          <ShieldAlert className="size-3.5" />
          <span>{error}</span>
        </div>
      )}

      <div
        data-testid="settings-permission-mode"
        data-forge-motion="settings-entry"
        className="forge-settings-permission-row"
      >
        <span
          className="forge-settings-provider-mark"
          data-configured={modeActive ? "true" : "false"}
          aria-hidden="true"
        >
          {fullAccessActive ? "全" : trustActive ? "信" : "手"}
        </span>
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <div className="truncate text-xs font-medium text-foreground">当前项目权限模式</div>
            <div className="truncate text-[11px] text-muted-foreground">
              {permissionModeLabel(modeState)}
            </div>
          </div>
          <div className="mt-1 text-[10px] text-muted-foreground">
            {trustAvailable
              ? activeWorkspacePath
              : "需要先打开一个项目会话"}
          </div>
        </div>
        <div className="forge-settings-permission-actions">
          <span
            className="forge-settings-status-pill"
            data-state={fullAccessActive ? "configured" : trustActive ? "configured" : "empty"}
          >
            {fullAccessActive ? "完全访问" : trustActive ? "已信任" : "手动确认"}
          </span>
          <ForgeButton
            size="xs"
            variant="outline"
            onClick={trustCurrentProject}
            disabled={modeBusy || !trustAvailable || trustActive}
            aria-label="信任当前项目"
          >
            <CheckCircle2 className="size-3" />
            信任当前项目
          </ForgeButton>
          <ForgeButton
            size="xs"
            variant="outline"
            onClick={fullAccessCurrentProject}
            disabled={modeBusy || !trustAvailable || fullAccessActive}
            aria-label="完全访问"
          >
            <ShieldAlert className="size-3" />
            完全访问
          </ForgeButton>
          <ForgeButton
            size="xs"
            variant="ghost"
            onClick={restoreManualConfirm}
            disabled={modeBusy || !activeSessionId || !modeActive}
            aria-label="恢复手动确认"
          >
            恢复手动确认
          </ForgeButton>
        </div>
      </div>

      <div className="forge-settings-permission-list">
        {rows.map((row) => (
          <div
            key={row.toolName}
            data-testid={`settings-permission-rule-${row.toolName}`}
            data-forge-motion="settings-entry"
            className="forge-settings-permission-row"
          >
            <span
              className="forge-settings-provider-mark"
              data-configured={row.decision === "allow" ? "true" : "false"}
              aria-hidden="true"
            >
              {row.decision === "deny" ? "!" : row.label.slice(0, 1)}
            </span>
            <div className="min-w-0">
              <div className="flex min-w-0 items-center gap-2">
                <div className="truncate text-xs font-medium text-foreground">{row.label}</div>
                <div className="truncate text-[11px] text-muted-foreground">{row.toolName}</div>
              </div>
              <div className="mt-1 text-[10px] text-muted-foreground">
                {row.hasRule ? "自定义规则" : "默认确认策略"}
              </div>
            </div>
            <div className="forge-settings-permission-actions">
              <span
                className="forge-settings-status-pill"
                data-state={row.decision === "allow" ? "configured" : row.decision === "deny" ? "denied" : "empty"}
              >
                {permissionDecisionLabel(row.decision)}
              </span>
              <ForgeButton
                size="xs"
                variant="outline"
                onClick={() => updateRule(row.toolName, "allow")}
                disabled={busyTool === row.toolName}
                aria-label={`允许 ${row.toolName}`}
              >
                <CheckCircle2 className="size-3" />
                允许
              </ForgeButton>
              <ForgeButton
                size="xs"
                variant="outline"
                onClick={() => updateRule(row.toolName, "deny")}
                disabled={busyTool === row.toolName}
                aria-label={`拒绝 ${row.toolName}`}
              >
                <XCircle className="size-3" />
                拒绝
              </ForgeButton>
              <ForgeButton
                size="xs"
                variant="ghost"
                onClick={() => resetRule(row.toolName)}
                disabled={busyTool === row.toolName || !row.hasRule}
                aria-label={`重置 ${row.toolName}`}
              >
                重置
              </ForgeButton>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

const manualModeState: PermissionModeState = {
  mode: "manual_confirm",
  workspace_path: null,
  session_scoped: true,
};

interface PermissionRow {
  toolName: string;
  label: string;
  decision: PermissionRuleDecision | "default";
  hasRule: boolean;
}

function buildPermissionRows(rules: PermissionRuleView[]): PermissionRow[] {
  const byTool = new Map(rules.map((rule) => [rule.tool_name, rule]));
  const toolNames = Array.from(new Set([...MANAGED_TOOLS, ...rules.map((rule) => rule.tool_name)])).sort();
  return toolNames.map((toolName) => {
    const rule = byTool.get(toolName);
    return {
      toolName,
      label: TOOL_LABELS[toolName] ?? toolName,
      decision: rule?.decision ?? "default",
      hasRule: Boolean(rule),
    };
  });
}

function permissionDecisionLabel(decision: PermissionRow["decision"]): string {
  if (decision === "allow") return "允许";
  if (decision === "deny") return "拒绝";
  return "默认";
}

function permissionModeLabel(state: PermissionModeState): string {
  if (state.mode === "full_access") return "完全访问";
  if (state.mode === "trust_current_project") return "信任当前项目";
  return "手动确认";
}

function formatPermissionError(event: unknown): string {
  return event instanceof Error ? event.message : String(event);
}
