import type { ComponentProps, ComponentType, ReactNode } from "react";
import {
  AlertCircle,
  Brain,
  Clock,
  Database,
  FolderOpen,
  Info,
  KeyRound,
  Settings,
  Stethoscope,
  UserRound,
  Wrench,
} from "lucide-react";
import { SettingsLocalDataSection } from "@/components/settings/SettingsLocalDataSection";
import { SettingsProviderRows } from "@/components/settings/SettingsProviderRows";
import { SettingsProviderSection } from "@/components/settings/SettingsProviderSection";
import { SettingsSummaryStrip } from "@/components/settings/SettingsSummaryStrip";
import { DiagnosticsPanel } from "@/components/settings/DiagnosticsPanel";
import { MemoryPanel } from "@/components/settings/MemoryPanel";
import { ProfilesPanel } from "@/components/settings/ProfilesPanel";
import { GeneralSettings } from "@/components/settings/GeneralSettings";
import { SchedulerPanel } from "@/components/settings/SchedulerPanel";

export type SettingsSectionId =
  | "general"
  | "models"
  | "workspace"
  | "tools"
  | "memory"
  | "profiles"
  | "scheduler"
  | "data"
  | "diagnostics"
  | "about";

type SettingsIcon = ComponentType<{ className?: string }>;

interface SettingsCenterShellProps {
  activeSection: SettingsSectionId;
  onSectionChange: (section: SettingsSectionId) => void;
  configuredCount: number;
  providerTotal: number;
  sessionCount: number;
  workspaceName: string;
  workspacePath: string;
  workspaceCount: number;
  providerLabel: string;
  modelLabel: string;
  providerRowsProps: ComponentProps<typeof SettingsProviderRows>;
  localDataProps: ComponentProps<typeof SettingsLocalDataSection>;
  error: string | null;
}

const SETTINGS_SECTIONS: Array<{
  id: SettingsSectionId;
  title: string;
  caption: string;
  icon: SettingsIcon;
}> = [
  { id: "general", title: "通用", caption: "服务与自启", icon: Settings },
  { id: "models", title: "模型服务", caption: "密钥与默认服务", icon: KeyRound },
  { id: "workspace", title: "工作区", caption: "当前项目环境", icon: FolderOpen },
  { id: "tools", title: "工具", caption: "本机执行通道", icon: Wrench },
  { id: "memory", title: "记忆", caption: "上下文与经验", icon: Brain },
  { id: "profiles", title: "资料", caption: "服务与工作区预设", icon: UserRound },
  { id: "scheduler", title: "调度", caption: "定时任务", icon: Clock },
  { id: "data", title: "本机数据", caption: "对话与缓存", icon: Database },
  { id: "diagnostics", title: "诊断", caption: "系统健康检查", icon: Stethoscope },
  { id: "about", title: "关于", caption: "Forge Workbench", icon: Info },
];

export function SettingsCenterShell({
  activeSection,
  onSectionChange,
  configuredCount,
  providerTotal,
  sessionCount,
  workspaceName,
  workspacePath,
  workspaceCount,
  providerLabel,
  modelLabel,
  providerRowsProps,
  localDataProps,
  error,
}: SettingsCenterShellProps) {
  const activeMeta = SETTINGS_SECTIONS.find((section) => section.id === activeSection) ?? SETTINGS_SECTIONS[0];
  const ActiveIcon = activeMeta.icon;

  return (
    <div className="forge-settings-center">
      <aside className="forge-settings-sidebar" aria-label="设置分类">
        <SettingsSummaryStrip
          configuredCount={configuredCount}
          providerTotal={providerTotal}
          sessionCount={sessionCount}
        />
        <nav className="forge-settings-nav">
          {SETTINGS_SECTIONS.map((section) => {
            const Icon = section.icon;
            const isActive = section.id === activeSection;

            return (
              <button
                key={section.id}
                type="button"
                className="forge-settings-nav-button"
                data-active={isActive ? "true" : "false"}
                aria-current={isActive ? "page" : undefined}
                onClick={() => onSectionChange(section.id)}
              >
                <span className="forge-settings-nav-icon" aria-hidden="true">
                  <Icon className="size-3.5" />
                </span>
                <span className="forge-settings-nav-copy">
                  <span className="forge-settings-nav-title">{section.title}</span>
                  <span className="forge-settings-nav-caption">{section.caption}</span>
                </span>
              </button>
            );
          })}
        </nav>
      </aside>

      <section className="forge-settings-content" aria-label={activeMeta.title}>
        <div className="forge-settings-content-header">
          <span className="forge-settings-content-icon" aria-hidden="true">
            <ActiveIcon className="size-4" />
          </span>
          <div className="min-w-0">
            <p className="forge-settings-kicker">{activeMeta.caption}</p>
            <h3 className="forge-settings-content-title">{activeMeta.title}</h3>
          </div>
        </div>

        <div className="forge-settings-panel-stack">
          {activeSection === "general" && <GeneralSettings />}

          {activeSection === "models" && (
            <>
              <SettingsReadOnlyPanel>
                <SettingsInfoList>
                  <SettingsInfoRow label="当前服务" value={providerLabel} />
                  <SettingsInfoRow label="默认模型" value={modelLabel} />
                  <SettingsInfoRow
                    label="密钥覆盖"
                    value={`${configuredCount}/${providerTotal} 个服务已配置`}
                  />
                </SettingsInfoList>
              </SettingsReadOnlyPanel>
              <SettingsProviderSection providerRowsProps={providerRowsProps} showHeading={false} />
            </>
          )}

          {activeSection === "workspace" && (
            <SettingsReadOnlyPanel
              title="项目环境"
              description="Forge 的对话、命令和上下文都绑定到当前项目。"
            >
              <SettingsInfoList>
                <SettingsInfoRow label="当前项目" value={workspaceName} />
                <SettingsInfoRow label="项目路径" value={workspacePath} subtle />
                <SettingsInfoRow label="保存项目" value={`${workspaceCount} 个`} />
              </SettingsInfoList>
            </SettingsReadOnlyPanel>
          )}

          {activeSection === "tools" && (
            <SettingsReadOnlyPanel title="执行通道" description="工具调用继续沿用当前项目的确认与输出流。">
              <SettingsInfoList>
                <SettingsInfoRow label="Shell" value="通过命令确认卡片执行" />
                <SettingsInfoRow label="文件操作" value="写入前保留确认边界" />
                <SettingsInfoRow label="能力入口" value="由能力面板管理" />
              </SettingsInfoList>
            </SettingsReadOnlyPanel>
          )}

          {activeSection === "memory" && <MemoryPanel />}

          {activeSection === "profiles" && <ProfilesPanel />}

          {activeSection === "scheduler" && <SchedulerPanel />}

          {activeSection === "data" && (
            <>
              <SettingsReadOnlyPanel>
                <SettingsInfoList>
                  <SettingsInfoRow label="保存位置" value="这台电脑" />
                  <SettingsInfoRow label="项目文件" value="清除对话不会删除项目文件" />
                </SettingsInfoList>
              </SettingsReadOnlyPanel>
              <SettingsLocalDataSection {...localDataProps} showHeading={false} />
            </>
          )}

          {activeSection === "diagnostics" && (
            <DiagnosticsPanel />
          )}

          {activeSection === "about" && (
            <SettingsReadOnlyPanel title="Forge Workbench">
              <SettingsInfoList>
                <SettingsInfoRow label="应用类型" value="Tauri 本机桌面应用" />
                <SettingsInfoRow label="工作方式" value="本机项目优先" />
                <SettingsInfoRow label="密钥策略" value="仅保存在本机配置" />
              </SettingsInfoList>
            </SettingsReadOnlyPanel>
          )}
        </div>

        {error && (
          <div className="forge-settings-error" role="alert">
            <AlertCircle className="size-3.5" />
            <span>{error}</span>
          </div>
        )}
      </section>
    </div>
  );
}

function SettingsReadOnlyPanel({
  title,
  description,
  children,
}: {
  title?: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="forge-settings-readonly-panel" data-forge-motion="settings-entry">
      {(title || description) && (
        <div className="forge-settings-readonly-heading">
          {title && <h4>{title}</h4>}
          {description && <p>{description}</p>}
        </div>
      )}
      {children}
    </div>
  );
}

function SettingsInfoList({ children }: { children: ReactNode }) {
  return <dl className="forge-settings-info-list">{children}</dl>;
}

function SettingsInfoRow({
  label,
  value,
  subtle = false,
}: {
  label: string;
  value: string;
  subtle?: boolean;
}) {
  return (
    <div className="forge-settings-info-row">
      <dt>{label}</dt>
      <dd data-subtle={subtle ? "true" : "false"}>{value}</dd>
    </div>
  );
}
