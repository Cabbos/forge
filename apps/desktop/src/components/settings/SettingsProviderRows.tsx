import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, Eye, EyeOff } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import { ForgeTextInput } from "@/components/primitives/input";
import { formatContextWindow, PROVIDERS } from "@/lib/providers";
import type { KeyStatus } from "@/lib/tauri";

interface SettingsProviderRowsProps {
  keys: KeyStatus[];
  editing: string | null;
  value: string;
  visible: boolean;
  saving: boolean;
  onEdit: (provider: string) => void;
  onValueChange: (value: string) => void;
  onVisibleChange: (visible: boolean) => void;
  onSave: () => void;
  onCancel: () => void;
  onRemove: (provider: string) => void;
}

export function SettingsProviderRows({
  keys,
  editing,
  value,
  visible,
  saving,
  onEdit,
  onValueChange,
  onVisibleChange,
  onSave,
  onCancel,
  onRemove,
}: SettingsProviderRowsProps) {
  return (
    <div data-testid="settings-preferences-panel" className="forge-settings-preferences-panel">
      {keys.map((key) => {
        const provider = PROVIDERS.find((item) => item.id === key.provider);
        const providerLabel = provider?.label ?? key.provider;
        const defaultModel = provider?.models.find((model) => model.id === provider.defaultModel);
        const defaultContext = formatContextWindow(defaultModel?.contextWindowTokens);

        return (
          <div
            key={key.provider}
            data-testid="settings-provider-row"
            data-forge-motion="settings-entry"
            data-configured={key.set}
            className="forge-settings-row"
          >
            <div
              className="forge-settings-provider-mark"
              data-configured={key.set ? "true" : "false"}
              aria-hidden="true"
            >
              {providerLabel.slice(0, 1)}
            </div>
            <div className="forge-settings-provider-copy min-w-0">
              <div className="flex min-w-0 items-center gap-2">
                <div className="truncate text-xs font-medium text-foreground">{providerLabel}</div>
                <div className="truncate text-[11px] text-muted-foreground">
                  {key.set ? "已连接" : "等待密钥"}
                </div>
              </div>
              {defaultModel && (
                <>
                  <div className="mt-1 truncate text-[11px] text-muted-foreground/85">
                    {defaultModel.name}
                  </div>
                  <div className="mt-0.5 text-[10px] text-muted-foreground">
                    {["默认模型", defaultContext && `上下文 ${defaultContext}`].filter(Boolean).join(" · ")}
                  </div>
                </>
              )}
            </div>

            <div className="forge-settings-row-side">
              <span
                data-testid="settings-provider-status"
                data-state={key.set ? "configured" : "empty"}
                className="forge-settings-status-pill"
                title={key.set ? key.preview : undefined}
              >
                {key.set ? "已配置" : "未配置"}
              </span>
              {editing !== key.provider && (
                <div className="flex items-center justify-end gap-2">
                  <ForgeButton size="xs" variant="outline" onClick={() => onEdit(key.provider)}>
                    {key.set ? "更新" : "添加"}
                  </ForgeButton>
                  {key.set && (
                    <ForgeButton
                      size="xs"
                      variant="ghost"
                      onClick={() => onRemove(key.provider)}
                      className="text-destructive hover:text-destructive"
                    >
                      移除
                    </ForgeButton>
                  )}
                </div>
              )}
            </div>

            {editing === key.provider && (
              <div className="forge-settings-edit-row">
                <div className="relative">
                  <ForgeTextInput
                    type={visible ? "text" : "password"}
                    value={value}
                    onChange={(event) => onValueChange(event.target.value)}
                    placeholder={provider?.keyPlaceholder ?? "sk-..."}
                    className="h-8 pr-9 text-xs"
                    autoFocus
                  />
                  <ButtonPrimitive
                    type="button"
                    onClick={() => onVisibleChange(!visible)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                    title={visible ? "隐藏密钥" : "显示密钥"}
                  >
                    {visible ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
                  </ButtonPrimitive>
                </div>
                <div className="flex gap-1.5">
                  <ForgeButton size="xs" onClick={onSave} disabled={saving}>
                    <Check className="size-3" />
                    保存
                  </ForgeButton>
                  <ForgeButton size="xs" variant="ghost" onClick={onCancel}>
                    取消
                  </ForgeButton>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
