import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, Eye, EyeOff, Pencil, RefreshCw } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import { ForgeTextInput } from "@/components/primitives/input";
import {
  deriveProviderEvidenceSummary,
  formatContextWindow,
  PROVIDERS,
  type ProviderDefinition,
  type ProviderEvidenceSummary,
  type ProviderModelCatalogSource,
  type ProviderProbeEvidence,
} from "@/lib/providers";
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
  onSetDefaultModel: (provider: string, model: string) => void;
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
  onSetDefaultModel,
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
        const cachedModelCatalogSource = provider?.modelCatalogSource
          ? [
              cachedModelCatalogSourceLabel(provider.modelCatalogSource),
              modelCatalogRecordedAtLabel(provider.modelCatalogRecordedAtMs),
            ].join(" · ")
          : "";
        const probeResult = probeResults[key.provider];
        const cachedProbeEvidence = probeResult ? null : provider?.probeEvidence ?? null;
        const evidenceSummary = provider ? deriveProviderEvidenceSummary(provider) : null;
        const probing = probingProvider === key.provider;
        const modelCatalogResult = modelCatalogResults[key.provider];
        const refreshingModels = refreshingModelsProvider === key.provider;
        const editableProviderProfile = provider?.source === "user_defined" || provider?.source === "user_override";
        const available = key.configured && key.status === "available";

        return (
          <div
            key={key.provider}
            data-testid="settings-provider-row"
            data-forge-motion="settings-entry"
            data-configured={available}
            className="forge-settings-row"
          >
            <div
              className="forge-settings-provider-mark"
              data-configured={available ? "true" : "false"}
              aria-hidden="true"
            >
              {providerLabel.slice(0, 1)}
            </div>
            <div className="forge-settings-provider-copy min-w-0">
              <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                <div
                  data-testid="settings-provider-readable-text"
                  data-provider-readable="label"
                  className="forge-settings-provider-readable-text text-xs font-medium text-foreground"
                >
                  {providerLabel}
                </div>
                <div className="truncate text-[11px] text-muted-foreground">
                  {available ? "已连接" : key.configured ? "凭据需修复" : "等待密钥"}
                </div>
              </div>
              {defaultModel && (
                <>
                  <div
                    data-testid="settings-provider-readable-text"
                    data-provider-readable="model"
                    className="forge-settings-provider-readable-text mt-1 text-[11px] text-muted-foreground/85"
                  >
                    {defaultModel.name}
                  </div>
                  <div
                    data-testid="settings-provider-readable-text"
                    data-provider-readable="meta"
                    className="forge-settings-provider-readable-text mt-0.5 text-[10px] text-muted-foreground"
                  >
                    {[
                      "默认模型",
                      defaultContext && `上下文 ${defaultContext}`,
                      provider?.requiresApiKey === false && "not required",
                      cachedModelCatalogSource,
                    ].filter(Boolean).join(" · ")}
                  </div>
                </>
              )}
            </div>

            <div className="forge-settings-row-side">
              <span
                data-testid="settings-provider-status"
                data-state={available ? "configured" : key.configured ? "error" : "empty"}
                className="forge-settings-status-pill"
                title={key.error ?? undefined}
              >
                {available ? "已配置" : key.configured ? "需修复" : "未配置"}
              </span>
              {editing !== key.provider && (
                <div className="flex flex-wrap items-center justify-end gap-2">
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
                    {key.configured ? "更新" : "添加"}
                  </ForgeButton>
                  {key.configured && (
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

            {evidenceSummary && (
              <div
                data-testid="settings-provider-evidence-summary"
                data-state={providerEvidenceBlockState(evidenceSummary)}
                className="forge-settings-provider-probe"
              >
                <div className="forge-settings-provider-probe-head">
                  <span
                    className="forge-settings-status-pill"
                    data-state={providerEvidencePillState(evidenceSummary)}
                  >
                    {evidenceSummary.label}
                  </span>
                  <span className="min-w-0 truncate text-[11px] font-medium text-foreground">
                    证据摘要
                  </span>
                </div>
                <div className="forge-settings-provider-probe-meta">
                  {evidenceSummary.detail}
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

            {cachedProbeEvidence && (
              <div
                data-testid="settings-provider-cached-probe"
                data-state={cachedProbeEvidence.status}
                className="forge-settings-provider-probe"
              >
                <div className="forge-settings-provider-probe-head">
                  <span
                    className="forge-settings-status-pill"
                    data-state={cachedProbeEvidence.status === "passed" ? "configured" : "denied"}
                  >
                    {cachedProbeStatusLabel(cachedProbeEvidence)}
                  </span>
                  <span className="min-w-0 truncate text-[11px] font-medium text-foreground">
                    manual probe evidence
                  </span>
                </div>
                <div className="forge-settings-provider-probe-meta">
                  {[
                    cachedProbeRecordedAtLabel(cachedProbeEvidence),
                    cachedProbeEvidence.model && `模型 ${cachedProbeEvidence.model}`,
                    cachedProbeEvidence.base_url && `Base ${cachedProbeEvidence.base_url}`,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
                {cachedProbeEvidence.checks.length > 0 && (
                  <div className="forge-settings-provider-probe-checks">
                    {cachedProbeEvidence.checks.map((check) => (
                      <span
                        key={check.id}
                        className="forge-settings-provider-probe-check"
                        data-state={check.status}
                      >
                        {check.label}
                      </span>
                    ))}
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
                    {modelCatalogResult.models.slice(0, 6).map((model) => {
                      const currentSelection = key.provider === selectedProvider && model.id === selectedModel;
                      const defaultSelection = provider?.defaultModel === model.id;
                      return (
                        <span key={model.id} className="inline-flex items-center gap-1">
                          <ButtonPrimitive
                            type="button"
                            className="forge-settings-provider-probe-check"
                            data-state={currentSelection ? "configured" : "passed"}
                            title={model.id}
                            aria-label={`使用模型 ${model.id}`}
                            onClick={() => onUseModel(key.provider, model.id)}
                          >
                            {model.name || model.id}
                          </ButtonPrimitive>
                          {editableProviderProfile && !defaultSelection && (
                            <ButtonPrimitive
                              type="button"
                              className="forge-settings-provider-probe-check"
                              data-state="passed"
                              title={`设为 Provider 默认：${model.id}`}
                              aria-label={`设为 Provider 默认 ${model.id}`}
                              onClick={() => onSetDefaultModel(key.provider, model.id)}
                            >
                              默认
                            </ButtonPrimitive>
                          )}
                        </span>
                      );
                    })}
                  </div>
                )}
                {modelCatalogResult.base_url && (
                  <div className="forge-settings-provider-probe-meta">
                    Base {modelCatalogResult.base_url}
                  </div>
                )}
                {modelCatalogResult.source && (
                  <div className="forge-settings-provider-probe-meta">
                    {[modelCatalogSourceLabel(modelCatalogResult.source), modelCatalogRecordedAtLabel(modelCatalogResult.recorded_at_ms)].join(" · ")}
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

function modelCatalogSourceLabel(source: ProviderModelCatalogSource) {
  switch (source) {
    case "live_endpoint":
      return "Live /models";
    case "static_fallback":
      return "Forge static catalog · not live-certified";
    case "unsupported":
      return "Catalog source unsupported";
  }
}

function cachedModelCatalogSourceLabel(source: ProviderModelCatalogSource) {
  switch (source) {
    case "live_endpoint":
      return "目录 Live /models";
    case "static_fallback":
      return "目录 Forge static catalog";
    case "unsupported":
      return "目录 unsupported";
  }
}

function modelCatalogRecordedAtLabel(recordedAtMs?: number | null) {
  if (typeof recordedAtMs !== "number" || !Number.isFinite(recordedAtMs)) {
    return "目录刷新时间未知";
  }
  const date = new Date(recordedAtMs);
  if (Number.isNaN(date.getTime())) return "目录刷新时间未知";
  return `目录刷新 ${date.toISOString().slice(0, 10)}`;
}

function cachedProbeStatusLabel(evidence: ProviderProbeEvidence) {
  return evidence.status === "passed" ? "上次手动检测通过" : "上次手动检测失败";
}

function cachedProbeRecordedAtLabel(evidence: ProviderProbeEvidence) {
  if (typeof evidence.recorded_at_ms !== "number" || !Number.isFinite(evidence.recorded_at_ms)) {
    return "检测时间未知";
  }
  const date = new Date(evidence.recorded_at_ms);
  if (Number.isNaN(date.getTime())) return "检测时间未知";
  return `检测 ${date.toISOString().slice(0, 10)}`;
}

function providerEvidenceBlockState(summary: ProviderEvidenceSummary) {
  if (summary.tone === "ready") return "passed";
  if (summary.tone === "blocked") return "failed";
  return undefined;
}

function providerEvidencePillState(summary: ProviderEvidenceSummary) {
  if (summary.tone === "blocked") return "denied";
  if (summary.tone === "ready") return "configured";
  return "empty";
}
