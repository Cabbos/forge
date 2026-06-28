import {
  ForgeDialog,
  ForgeDialogContent,
  ForgeDialogDescription,
  ForgeDialogHeader,
  ForgeDialogTitle,
  ForgeDialogTrigger,
} from "@/components/primitives/dialog";
import { ForgeButton } from "@/components/primitives/button";
import { Settings } from "lucide-react";
import {
  SettingsCenterShell,
  type SettingsSectionId,
} from "@/components/settings/SettingsCenterShell";
import { SettingsSummaryStrip } from "@/components/settings/SettingsSummaryStrip";
import { SettingsProviderSection } from "@/components/settings/SettingsProviderSection";
import { SettingsLocalDataSection } from "@/components/settings/SettingsLocalDataSection";
import { useSettingsDialogController } from "@/components/settings/useSettingsDialogController";

interface SettingsDialogProps {
  triggerClassName?: string;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  hideTrigger?: boolean;
  requestedSection?: SettingsSectionId | null;
}

export function SettingsDialog({
  triggerClassName,
  open,
  onOpenChange,
  hideTrigger = false,
  requestedSection = null,
}: SettingsDialogProps = {}) {
  const {
    dialogOpen,
    setDialogOpen,
    dialogRef,
    activeSection,
    setActiveSection,
    configuredCount,
    providerTotal,
    sessionCount,
    workspaceName,
    workspacePath,
    workspaceCount,
    providerLabel,
    modelLabel,
    providerRowsProps,
    profileEditorProps,
    localDataProps,
    error,
  } = useSettingsDialogController({ open, onOpenChange, requestedSection });

  return (
    <ForgeDialog open={dialogOpen} onOpenChange={setDialogOpen}>
      {!hideTrigger && (
        <ForgeDialogTrigger
          render={<ForgeButton variant="ghost" size="icon-sm" aria-label="设置" title="设置" className={triggerClassName} />}
        >
          <Settings className="size-4" />
        </ForgeDialogTrigger>
      )}
      <ForgeDialogContent ref={dialogRef} data-forge-motion="settings-dialog" className="forge-settings-dialog sm:max-w-[860px]">
        <ForgeDialogHeader className="forge-settings-header">
          <ForgeDialogTitle className="forge-settings-title">
            <Settings className="size-4" />
            设置
          </ForgeDialogTitle>
          <ForgeDialogDescription>
            管理模型服务和本机对话。密钥只保存在这台电脑。
          </ForgeDialogDescription>
        </ForgeDialogHeader>

        <SettingsCenterShell
          activeSection={activeSection}
          onSectionChange={setActiveSection}
          configuredCount={configuredCount}
          providerTotal={providerTotal}
          workspaceName={workspaceName}
          workspacePath={workspacePath}
          workspaceCount={workspaceCount}
          providerLabel={providerLabel}
          modelLabel={modelLabel}
          error={error}
          summaryStrip={
            <SettingsSummaryStrip
              configuredCount={configuredCount}
              providerTotal={providerTotal}
              sessionCount={sessionCount}
            />
          }
          providerSection={
            <SettingsProviderSection
              providerRowsProps={providerRowsProps}
              profileEditorProps={profileEditorProps}
              showHeading={false}
            />
          }
          localDataSection={
            <SettingsLocalDataSection {...localDataProps} showHeading={false} />
          }
        />
      </ForgeDialogContent>
    </ForgeDialog>
  );
}

export type { SettingsSectionId };
