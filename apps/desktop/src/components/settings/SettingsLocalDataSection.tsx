import { Trash2 } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";

interface SettingsLocalDataSectionProps {
  sessionCount: number;
  cleared: boolean;
  onClearAll: () => void;
  showHeading?: boolean;
}

export function SettingsLocalDataSection({
  sessionCount,
  cleared,
  onClearAll,
  showHeading = true,
}: SettingsLocalDataSectionProps) {
  return (
    <section className="forge-settings-section space-y-2">
      {showHeading && (
        <div className="forge-settings-heading">
          <Trash2 className="size-3.5 text-muted-foreground" />
          <h3 className="text-sm font-medium text-foreground">本机数据</h3>
        </div>
      )}
      <div data-forge-motion="settings-entry" className="forge-settings-danger-zone">
        <p className="text-xs leading-relaxed text-muted-foreground">
          清除这台电脑保存的对话列表，不会删除项目文件。
        </p>
        <ForgeButton
          size="sm"
          variant="destructive"
          onClick={onClearAll}
          disabled={sessionCount === 0}
        >
          <Trash2 className="size-3.5" />
          {cleared ? "已清除" : `清除本机对话（${sessionCount}）`}
        </ForgeButton>
      </div>
    </section>
  );
}
