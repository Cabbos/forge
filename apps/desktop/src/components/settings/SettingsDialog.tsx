import {
  ForgeDialog,
  ForgeDialogContent,
  ForgeDialogDescription,
  ForgeDialogHeader,
  ForgeDialogTitle,
  ForgeDialogTrigger,
} from "@/components/primitives/dialog";
import { ForgeButton } from "@/components/primitives/button";
import { Settings, AlertCircle } from "lucide-react";
import { SettingsLocalDataSection } from "@/components/settings/SettingsLocalDataSection";
import { SettingsProviderSection } from "@/components/settings/SettingsProviderSection";
import { SettingsSummaryStrip } from "@/components/settings/SettingsSummaryStrip";
import { useSettingsDialogController } from "@/components/settings/useSettingsDialogController";

interface SettingsDialogProps {
  triggerClassName?: string;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  hideTrigger?: boolean;
}

export function SettingsDialog({ triggerClassName, open, onOpenChange, hideTrigger = false }: SettingsDialogProps = {}) {
  const {
    dialogOpen,
    setDialogOpen,
    dialogRef,
    configuredCount,
    providerTotal,
    sessionCount,
    error,
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
      <ForgeDialogContent ref={dialogRef} data-forge-motion="settings-dialog" className="forge-settings-dialog sm:max-w-[590px]">
        <ForgeDialogHeader>
          <ForgeDialogTitle className="forge-settings-title">
            <Settings className="size-4" />
            设置
          </ForgeDialogTitle>
          <ForgeDialogDescription>
            管理模型服务和本机对话。密钥只保存在这台电脑。
          </ForgeDialogDescription>
        </ForgeDialogHeader>

        <SettingsSummaryStrip
          configuredCount={configuredCount}
          providerTotal={providerTotal}
          sessionCount={sessionCount}
        />

        <SettingsProviderSection providerRowsProps={providerRowsProps} />

        <SettingsLocalDataSection {...localDataProps} />

        {error && (
          <div className="flex items-center gap-1.5 text-xs text-destructive">
            <AlertCircle className="size-3" />
            {error}
          </div>
        )}
      </ForgeDialogContent>
    </ForgeDialog>
  );
}
