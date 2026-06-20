import { useMemo } from "react";
import { Plus } from "lucide-react";
import { ForgeButton } from "@/components/primitives/button";
import { ForgeTextInput } from "@/components/primitives/input";
import type { ProviderProfileInput, ProviderTransportName } from "@/lib/tauri";

export interface ProviderProfileDraft {
  id: string;
  label: string;
  transport: ProviderTransportName;
  baseUrl: string;
  apiKeyEnv: string;
  baseUrlEnv: string;
  defaultModel: string;
  aliases: string;
  noApiKey: boolean;
  supportsTools: boolean;
  supportsStreaming: boolean;
}

interface ProviderProfileEditorProps {
  open: boolean;
  draft: ProviderProfileDraft;
  saving: boolean;
  onOpenChange: (open: boolean) => void;
  onDraftChange: (draft: ProviderProfileDraft) => void;
  onSave: () => void;
}

export const EMPTY_PROVIDER_PROFILE_DRAFT: ProviderProfileDraft = {
  id: "",
  label: "",
  transport: "openai_chat_completions",
  baseUrl: "",
  apiKeyEnv: "",
  baseUrlEnv: "",
  defaultModel: "",
  aliases: "",
  noApiKey: false,
  supportsTools: true,
  supportsStreaming: true,
};

export function providerProfileInputFromDraft(draft: ProviderProfileDraft): ProviderProfileInput {
  const id = draft.id.trim();
  const apiKeyEnv = draft.noApiKey ? [] : splitList(draft.apiKeyEnv || defaultEnvName(id, "API_KEY"));
  return {
    id,
    label: draft.label.trim(),
    transport: draft.transport,
    base_url: draft.baseUrl.trim() || null,
    api_key_env: apiKeyEnv,
    base_url_env: splitList(draft.baseUrlEnv),
    default_model: draft.defaultModel.trim(),
    aliases: splitList(draft.aliases),
    supports_tools: draft.supportsTools,
    supports_streaming: draft.supportsStreaming,
  };
}

export function ProviderProfileEditor({
  open,
  draft,
  saving,
  onOpenChange,
  onDraftChange,
  onSave,
}: ProviderProfileEditorProps) {
  const canSave = useMemo(
    () => Boolean(draft.id.trim() && draft.label.trim() && draft.defaultModel.trim()),
    [draft.defaultModel, draft.id, draft.label],
  );

  const update = <K extends keyof ProviderProfileDraft>(key: K, value: ProviderProfileDraft[K]) => {
    onDraftChange({ ...draft, [key]: value });
  };

  return (
    <div className="space-y-2" data-testid="provider-profile-editor">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <h4 className="forge-settings-panel-title">自定义 Provider</h4>
        </div>
        <ForgeButton size="xs" variant="outline" onClick={() => onOpenChange(!open)}>
          <Plus className="size-3" />
          新增自定义 Provider
        </ForgeButton>
      </div>

      {open && (
        <div className="forge-settings-row grid gap-3">
          <div className="grid gap-2 sm:grid-cols-2">
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              Provider ID
              <ForgeTextInput
                value={draft.id}
                onChange={(event) => update("id", event.target.value)}
                placeholder="local-openai"
                className="h-8 text-xs"
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              显示名称
              <ForgeTextInput
                value={draft.label}
                onChange={(event) => update("label", event.target.value)}
                placeholder="Local OpenAI"
                className="h-8 text-xs"
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              Base URL
              <ForgeTextInput
                value={draft.baseUrl}
                onChange={(event) => update("baseUrl", event.target.value)}
                placeholder="http://127.0.0.1:1234/v1"
                className="h-8 text-xs"
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              默认模型
              <ForgeTextInput
                value={draft.defaultModel}
                onChange={(event) => update("defaultModel", event.target.value)}
                placeholder="local-model"
                className="h-8 text-xs"
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              传输协议
              <select
                value={draft.transport}
                onChange={(event) => update("transport", event.target.value as ProviderTransportName)}
                className="h-8 rounded-md border border-border bg-background px-2 text-xs text-foreground"
              >
                <option value="openai_chat_completions">OpenAI-compatible</option>
                <option value="anthropic_messages">Anthropic-compatible</option>
              </select>
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              API Key env
              <ForgeTextInput
                value={draft.apiKeyEnv}
                onChange={(event) => update("apiKeyEnv", event.target.value)}
                placeholder={defaultEnvName(draft.id, "API_KEY")}
                className="h-8 text-xs"
                disabled={draft.noApiKey}
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              Base URL env
              <ForgeTextInput
                value={draft.baseUrlEnv}
                onChange={(event) => update("baseUrlEnv", event.target.value)}
                placeholder={defaultEnvName(draft.id, "BASE_URL")}
                className="h-8 text-xs"
              />
            </label>
            <label className="grid gap-1 text-[11px] text-muted-foreground">
              Aliases
              <ForgeTextInput
                value={draft.aliases}
                onChange={(event) => update("aliases", event.target.value)}
                placeholder="local, lmstudio"
                className="h-8 text-xs"
              />
            </label>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <label className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <input
                type="checkbox"
                checked={draft.noApiKey}
                onChange={(event) => update("noApiKey", event.currentTarget.checked)}
              />
              不需要 API Key
            </label>
            <label className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <input
                type="checkbox"
                checked={draft.supportsTools}
                onChange={(event) => update("supportsTools", event.currentTarget.checked)}
              />
              支持工具调用
            </label>
            <label className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <input
                type="checkbox"
                checked={draft.supportsStreaming}
                onChange={(event) => update("supportsStreaming", event.currentTarget.checked)}
              />
              支持流式输出
            </label>
          </div>
          <div className="flex justify-end gap-2">
            <ForgeButton size="xs" variant="ghost" onClick={() => onOpenChange(false)}>
              取消
            </ForgeButton>
            <ForgeButton size="xs" onClick={onSave} disabled={!canSave || saving}>
              保存 Provider
            </ForgeButton>
          </div>
        </div>
      )}
    </div>
  );
}

function splitList(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function defaultEnvName(id: string, suffix: string): string {
  const normalized = id.trim().toUpperCase().replace(/[^A-Z0-9]+/g, "_").replace(/^_+|_+$/g, "");
  return normalized ? `${normalized}_${suffix}` : suffix;
}
