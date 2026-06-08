import type { ReactNode } from "react";
import { Database, ShieldCheck, Sparkles } from "lucide-react";

interface SettingsSummaryStripProps {
  configuredCount: number;
  providerTotal: number;
  sessionCount: number;
}

export function SettingsSummaryStrip({
  configuredCount,
  providerTotal,
  sessionCount,
}: SettingsSummaryStripProps) {
  return (
    <div data-testid="settings-summary-strip" className="forge-settings-summary-strip" aria-label="设置摘要">
      <SettingsSummaryItem
        icon={<Sparkles className="size-3.5" />}
        label="模型服务"
        value={`${configuredCount}/${providerTotal} 已配置`}
      />
      <SettingsSummaryItem
        icon={<Database className="size-3.5" />}
        label="本机对话"
        value={`${sessionCount} 个`}
      />
      <SettingsSummaryItem
        icon={<ShieldCheck className="size-3.5" />}
        label="密钥存储"
        value="仅本机"
      />
    </div>
  );
}

function SettingsSummaryItem({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div data-forge-motion="settings-entry" className="forge-settings-summary-item">
      <span className="forge-settings-summary-icon">{icon}</span>
      <span className="forge-settings-summary-copy">
        <span className="forge-settings-summary-label">{label}</span>
        <span className="forge-settings-summary-value">{value}</span>
      </span>
    </div>
  );
}
