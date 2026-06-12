import type { ReactNode } from "react";
import { X } from "lucide-react";
import { ForgeIcon } from "@/components/primitives/icon";
import { capabilityIconMeta } from "@/lib/capability-icons";
import type { EcosystemItemStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const STATUS_LABELS: Record<EcosystemItemStatus, string> = {
  healthy: "正常",
  unavailable: "不可用",
  warning: "警告",
  unknown: "未知",
};

interface CapabilityDetailDrawerProps {
  open: boolean;
  onClose: () => void;
  id: string;
  name: string;
  description: string;
  kind: string;
  source: string;
  version: string;
  enabled: boolean;
  status?: EcosystemItemStatus;
  statusMessage?: string | null;
  configurable?: boolean;
  configSummary?: string | null;
  children?: ReactNode;
}

export function CapabilityDetailDrawer({
  open,
  onClose,
  id,
  name,
  description,
  kind,
  source,
  version,
  enabled,
  status,
  statusMessage,
  configurable,
  configSummary,
  children,
}: CapabilityDetailDrawerProps) {
  if (!open) return null;

  const meta = capabilityIconMeta(kind);

  return (
    <div
      className="forge-capability-drawer"
      role="dialog"
      aria-label={`${name} 详情`}
    >
      <div className="forge-capability-drawer-header">
        <ForgeIcon icon={meta.icon} tone={meta.tone} disabled={!enabled} />
        <div className="forge-capability-drawer-title">
          <h4>{name}</h4>
          <span className="forge-capability-drawer-id">{id}</span>
        </div>
        <button
          type="button"
          className="forge-capability-drawer-close"
          onClick={onClose}
          aria-label="关闭详情"
        >
          <X size={16} />
        </button>
      </div>

      <div className="forge-capability-drawer-body">
        <dl className="forge-capability-drawer-fields">
          <dt>描述</dt>
          <dd>{description || "无描述"}</dd>

          <dt>类型</dt>
          <dd>
            {kind === "skill" && "插件"}
            {kind === "tool" && "工具"}
            {kind === "hook" && "自动化"}
            {kind === "mcp_server" && "MCP 连接"}
          </dd>

          <dt>来源</dt>
          <dd className={cn(kind === "mcp_server" && "forge-capability-description-mono")}>
            {source || "内置"}
          </dd>

          <dt>版本</dt>
          <dd>{version || "—"}</dd>

          <dt>状态</dt>
          <dd>
            <span
              className={cn(
                "forge-capability-status-badge",
                enabled ? "enabled" : "disabled",
              )}
            >
              {enabled ? "已启用" : "已停用"}
            </span>
            {status && (
              <span
                className="forge-capability-status-badge"
                data-status={status}
                style={{ marginLeft: 8 }}
              >
                {STATUS_LABELS[status]}
              </span>
            )}
          </dd>

          {statusMessage && (
            <>
              <dt>状态说明</dt>
              <dd>{statusMessage}</dd>
            </>
          )}

          {typeof configurable === "boolean" && (
            <>
              <dt>界面配置</dt>
              <dd>
                {configurable ? (
                  configSummary ? (
                    <span>{configSummary}</span>
                  ) : (
                    <span>支持界面配置</span>
                  )
                ) : (
                  <span className="forge-capability-config-hint">
                    暂不支持界面配置 — 请通过配置文件或 CLI 管理此项目
                  </span>
                )}
              </dd>
            </>
          )}
        </dl>

        {children}
      </div>
    </div>
  );
}
