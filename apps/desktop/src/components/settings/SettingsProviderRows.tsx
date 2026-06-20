import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, Eye, EyeOff, Pencil, RefreshCw } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import { ForgeTextInput } from "@/components/primitives/input";
import { formatContextWindow, PROVIDERS, type ProviderDefinition } from "@/lib/providers";
import type { KeyStatus } from "@/lib/tauri";
import type { ProviderModelCatalogResult, ProviderProbeResult } from "@/lib/tauri";

interface SettingsProviderRowsProps {
  keys: KeyStatus[];
  providers?: ProviderDefinition[];
  editing: string | null;
  value: string;
  visible: boolean;
  saving: boolean;
  probingProvider: string | null;
  probeResults: Record<string, ProviderProbeResult>;
  refreshingModelsProvider: string | null;
  modelCatalogResults: Record<string, ProviderModelCatalogResult>;
  selectedProvider: string;
  selectedModel: string;
  onEdit: (provider: string) => void;
  onValueChange: (value: string) => void;
  onVisibleChange: (visible: boolean) => void;
  onSave: () => void;
  onCancel: () => void;
  onRemove: (provider: string) => void;
  onProbe: (provider: string) => void;
  onRefreshModels: (provider: string) => void;
  onUseModel: (provider: string, model: string) => void;
  onEditProviderProfile: (provider: string) => void;
  onDeleteProviderProfile: (provider: string) => void;
}

export function SettingsProviderRows({
  keys,
  providers = PROVIDERS,
  editing,
  value,
  visible,
  saving,
  probingProvider,
  probeResults,
  refreshingModelsProvider,
  modelCatalogResults,
  selectedProvider,
  selectedModel,
  onEdit,
  onValueChange,
  onVisibleChange,
  onSave,
  onCancel,
  onRemove,
  onProbe,
  onRefreshModels,
  onUseModel,
  onEditProviderProfile,
  onDeleteProviderProfile,
}: SettingsProviderRowsProps) {
  const probeBusy = probingProvider !== null;
  const modelRefreshBusy = refreshingModelsProvider !== null;

  return (
    <div data-testid="settings-preferences-panel" className="forge-settings-preferences-panel">
      {keys.map((key) => {
        const provider = providers.find((item) => item.id === key.provider);
        const providerLabel = provider?.label ?? key.provider;
        const defaultModel = provider?.models.find((model) => model.id === provider.defaultModel);
        const defaultContext = formatContextWindow(defaultModel?.contextWindowTokens);
        const probeResult = probeResults[key.provider];
        const probing = probingProvider === key.provider;
        const modelCatalogResult = modelCatalogResults[key.provider];
        const refreshingModels = refreshingModelsProvider === key.provider;
        const editableProviderProfile = provider?.source === "user_defined" || provider?.source === "user_override";

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
                    {[
                      "默认模型",
                      defaultContext && `上下文 ${defaultContext}`,
                      provider?.requiresApiKey === false && "not required",
                    ].filter(Boolean).join(" · ")}
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
                  <ForgeButton
                    size="xs"
                    variant="outline"
                    onClick={() => onProbe(key.provider)}
                    disabled={probeBusy || modelRefreshBusy}
                    aria-label={`检测 ${providerLabel}`}
                    title={`检测 ${providerLabel}`}
                  >
                    {probing ? "检测中" : "检测"}
                  </ForgeButton>
                  <ForgeButton
                    size="xs"
                    variant="outline"
                    onClick={() => onRefreshModels(key.provider)}
                    disabled={probeBusy || modelRefreshBusy}
                    aria-label={`刷新模型 ${providerLabel}`}
                    title={`刷新模型 ${providerLabel}`}
                  >
                    <RefreshCw className={refreshingModels ? "size-3 animate-spin" : "size-3"} />
                    {refreshingModels ? "刷新中" : "模型"}
                  </ForgeButton>
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
                  {editableProviderProfile && (
                    <>
                      <ForgeButton
                        size="xs"
                        variant="ghost"
                        onClick={() => onEditProviderProfile(key.provider)}
                        aria-label={`编辑 Provider ${providerLabel}`}
                      >
                        <Pencil className="size-3" />
                        编辑 Provider
                      </ForgeButton>
                      <ForgeButton
                        size="xs"
                        variant="ghost"
                        onClick={() => onDeleteProviderProfile(key.provider)}
                        className="text-destructive hover:text-destructive"
                        aria-label={`删除 Provider ${providerLabel}`}
                      >
                        删除 Provider
                      </ForgeButton>
                    </>
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

            {probeResult && (
              <div
                data-testid="settings-provider-probe-result"
                data-state={probeResult.status}
                className="forge-settings-provider-probe"
              >
                <div className="forge-settings-provider-probe-head">
                  <span
                    className="forge-settings-status-pill"
                    data-state={probeResult.status === "passed" ? "configured" : "denied"}
                  >
                    {probeResult.status === "passed" ? "探测通过" : "探测失败"}
                  </span>
                  <span className="min-w-0 truncate text-[11px] font-medium text-foreground">
                    {probeResult.message}
                  </span>
                </div>
                <div className="forge-settings-provider-probe-meta">
                  {[probeResult.model && `模型 ${probeResult.model}`, probeResult.base_url && `Base ${probeResult.base_url}`]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
                {probeResult.checks.length > 0 && (
                  <div className="forge-settings-provider-probe-checks">
                    {probeResult.checks.map((check) => (
                      <span
                        key={check.id}
                        className="forge-settings-provider-probe-check"
                        data-state={check.status}
                        title={check.message}
                      >
                        {check.label}
                      </span>
                    ))}
                  </div>
                )}
                {probeResult.checks.some((check) => check.status === "failed") && (
                  <div className="forge-settings-provider-probe-message">
                    {probeResult.checks.find((check) => check.status === "failed")?.message}
                  </div>
                )}
                {probeResult.remediation && (
                  <div className="forge-settings-provider-probe-message">
                    {probeResult.remediation}
                  </div>
                )}
              </div>
            )}

            {modelCatalogResult && (
              <div
                data-testid="settings-provider-model-catalog-result"
                data-state={modelCatalogResult.status}
                className="forge-settings-provider-probe"
              >
                <div className="forge-settings-provider-probe-head">
                  <span
                    className="forge-settings-status-pill"
                    data-state={modelCatalogResult.status === "available" ? "configured" : "denied"}
                  >
                    {modelCatalogResult.status === "available" ? "模型已刷新" : "模型刷新失败"}
                  </span>
                  <span className="min-w-0 truncate text-[11px] font-medium text-foreground">
                    {modelCatalogResult.message}
                  </span>
                </div>
                {modelCatalogResult.models.length > 0 && (
                  <div className="forge-settings-provider-probe-checks">
                    {modelCatalogResult.models.slice(0, 6).map((model) => (
                      <ButtonPrimitive
                        key={model.id}
                        type="button"
                        className="forge-settings-provider-probe-check"
                        data-state={key.provider === selectedProvider && model.id === selectedModel ? "configured" : "passed"}
                        title={model.id}
                        aria-label={`使用模型 ${model.id}`}
                        onClick={() => onUseModel(key.provider, model.id)}
                      >
                        {model.name || model.id}
                      </ButtonPrimitive>
                    ))}
                  </div>
                )}
                {modelCatalogResult.base_url && (
                  <div className="forge-settings-provider-probe-meta">
                    Base {modelCatalogResult.base_url}
                  </div>
                )}
                {modelCatalogResult.remediation && (
                  <div className="forge-settings-provider-probe-message">
                    {modelCatalogResult.remediation}
                  </div>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
