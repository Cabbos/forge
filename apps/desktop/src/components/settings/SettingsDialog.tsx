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
import { useState } from "react";
import {
  SettingsCenterShell,
  type SettingsSectionId,
} from "@/components/settings/SettingsCenterShell";
import { useSettingsDialogController } from "@/components/settings/useSettingsDialogController";

interface SettingsDialogProps {
  triggerClassName?: string;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  hideTrigger?: boolean;
}

export function SettingsDialog({ triggerClassName, open, onOpenChange, hideTrigger = false }: SettingsDialogProps = {}) {
  const [activeSection, setActiveSection] = useState<SettingsSectionId>("models");
  const {
    dialogOpen,
    setDialogOpen,
    dialogRef,
    configuredCount,
    providerTotal,
    sessionCount,
    error,
    workspaceName,
    workspacePath,
    workspaceCount,
    providerLabel,
    modelLabel,
    providerRowsProps,
    localDataProps,
  } = useSettingsDialogController({ open, onOpenChange });

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
          sessionCount={sessionCount}
          workspaceName={workspaceName}
          workspacePath={workspacePath}
          workspaceCount={workspaceCount}
          providerLabel={providerLabel}
          modelLabel={modelLabel}
          providerRowsProps={providerRowsProps}
          localDataProps={localDataProps}
          error={error}
        />
      </ForgeDialogContent>
    </ForgeDialog>
  );
}
