import type { ComponentProps } from "react";
import { Key } from "lucide-react";
import { SettingsProviderRows } from "@/components/settings/SettingsProviderRows";

interface SettingsProviderSectionProps {
  providerRowsProps: ComponentProps<typeof SettingsProviderRows>;
  showHeading?: boolean;
}

export function SettingsProviderSection({ providerRowsProps, showHeading = true }: SettingsProviderSectionProps) {
  return (
    <section className="forge-settings-section space-y-2">
      {showHeading && (
        <div className="forge-settings-heading">
          <Key className="size-3.5 text-muted-foreground" />
          <h3 className="text-sm font-medium text-foreground">模型服务</h3>
        </div>
      )}
      <SettingsProviderRows {...providerRowsProps} />
    </section>
  );
}
