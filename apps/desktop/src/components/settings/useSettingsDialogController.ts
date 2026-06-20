import { useCallback, useEffect, useState } from "react";
import type { ComponentProps } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { SettingsSectionId } from "@/components/settings/SettingsCenterShell";
import { SettingsLocalDataSection } from "@/components/settings/SettingsLocalDataSection";
import { SettingsProviderRows } from "@/components/settings/SettingsProviderRows";
import { buildSettingsProviderState } from "@/components/settings/SettingsDialogModel";
import { useSettingsDialogMotion } from "@/components/settings/useSettingsDialogMotion";
import { deleteSession, listProviderModels, probeProvider, setApiKey } from "@/lib/tauri";
import type { ProviderModelCatalogResult, ProviderProbeResult } from "@/lib/tauri";
import { useApiKeyStatusQuery } from "@/hooks/queries/useApiKeyStatusQuery";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { useProviderCatalog } from "@/hooks/queries/useProviderCatalogQuery";
import { getModelLabel, getProviderLabel } from "@/lib/providers";
import { useStore } from "@/store";

interface UseSettingsDialogControllerOptions {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function useSettingsDialogController({
  open,
  onOpenChange,
}: UseSettingsDialogControllerOptions = {}) {
  const [internalOpen, setInternalOpen] = useState(false);
  const [activeSection, setActiveSection] = useState<SettingsSectionId>("models");
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [saving, setSaving] = useState(false);
  const [probingProvider, setProbingProvider] = useState<string | null>(null);
  const [probeResults, setProbeResults] = useState<Record<string, ProviderProbeResult>>({});
  const [refreshingModelsProvider, setRefreshingModelsProvider] = useState<string | null>(null);
  const [modelCatalogResults, setModelCatalogResults] = useState<Record<string, ProviderModelCatalogResult>>({});
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);
  const queryClient = useQueryClient();
  const sessions = useStore((s) => s.sessions);
  const removeSession = useStore((s) => s.removeSession);
  const activeWorkspace = useStore((s) => s.activeWorkspaceId ? s.workspaces.get(s.activeWorkspaceId) ?? null : null);
  const workspaceCount = useStore((s) => s.workspaces.size);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const dialogOpen = open ?? internalOpen;
  const setDialogOpen = useCallback((nextOpen: boolean) => {
    if (open === undefined) setInternalOpen(nextOpen);
    onOpenChange?.(nextOpen);
  }, [onOpenChange, open]);
  const dialogRef = useSettingsDialogMotion(dialogOpen);
  const {
    data: keys = [],
    isError: keysIsError,
    error: keysError,
  } = useApiKeyStatusQuery(dialogOpen);
  const queryError = getQueryErrorMessage(keysIsError ? keysError : null);
  const providers = useProviderCatalog(dialogOpen);

  useEffect(() => {
    const openSettings = () => setDialogOpen(true);
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        setDialogOpen(true);
      }
    };

    window.addEventListener("forge:open-settings", openSettings);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("forge:open-settings", openSettings);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [setDialogOpen]);

  const handleClearAll = useCallback(async () => {
    // Remove all sessions from the backend source of truth, then clear the UI projection.
    for (const [id] of sessions) {
      await deleteSession(id).catch(() => {});
      removeSession(id);
    }
    await queryClient.invalidateQueries({ queryKey: queryKeys.sessions });
    setCleared(true);
    setTimeout(() => setCleared(false), 3000);
  }, [removeSession, sessions, queryClient]);

  const handleSave = useCallback(async () => {
    if (!editing) return;
    setSaving(true);
    setError(null);
    try {
      await setApiKey(editing, value);
      setProbeResults((previous) => {
        const next = { ...previous };
        delete next[editing];
        return next;
      });
      setEditing(null);
      setValue("");
      await queryClient.invalidateQueries({ queryKey: queryKeys.apiKeyStatus });
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  }, [editing, queryClient, value]);

  const handleRemove = useCallback(async (provider: string) => {
    setSaving(true);
    setError(null);
    try {
      await setApiKey(provider, "");
      setProbeResults((previous) => {
        const next = { ...previous };
        delete next[provider];
        return next;
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.apiKeyStatus });
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  }, [queryClient]);

  const handleEdit = useCallback((provider: string) => {
    setEditing(provider);
    setValue("");
    setError(null);
  }, []);

  const handleCancelEdit = useCallback(() => {
    setEditing(null);
    setValue("");
    setError(null);
  }, []);

  const handleProbe = useCallback(async (provider: string) => {
    setProbingProvider(provider);
    setError(null);
    try {
      const result = await probeProvider(provider);
      setProbeResults((previous) => ({ ...previous, [provider]: result }));
    } catch (e) {
      setProbeResults((previous) => ({
        ...previous,
        [provider]: {
          provider,
          provider_label: getProviderLabel(provider, providers),
          model: null,
          base_url: null,
          status: "failed",
          checks: [],
          message: String(e),
          remediation: null,
        },
      }));
    } finally {
      setProbingProvider(null);
    }
  }, [providers]);

  const handleRefreshModels = useCallback(async (provider: string) => {
    setRefreshingModelsProvider(provider);
    setError(null);
    try {
      const result = await listProviderModels(provider);
      setModelCatalogResults((previous) => ({ ...previous, [provider]: result }));
      if (result.status === "available") {
        void queryClient.invalidateQueries({ queryKey: queryKeys.providerCatalog });
      }
    } catch (e) {
      setModelCatalogResults((previous) => ({
        ...previous,
        [provider]: {
          provider,
          provider_label: getProviderLabel(provider, providers),
          base_url: null,
          status: "unavailable",
          models: [],
          message: String(e),
          remediation: null,
        },
      }));
    } finally {
      setRefreshingModelsProvider(null);
    }
  }, [providers]);

  const { sortedKeys, configuredCount, providerTotal } = buildSettingsProviderState(keys, providers);
  const sessionCount = sessions.size;
  const workspaceName = activeWorkspace?.name ?? "未选择项目";
  const workspacePath = activeWorkspace?.path ?? "打开项目后绑定工作区设置";

  const providerRowsProps: ComponentProps<typeof SettingsProviderRows> = {
    keys: sortedKeys,
    providers,
    editing,
    value,
    visible,
    saving,
    probingProvider,
    probeResults,
    refreshingModelsProvider,
    modelCatalogResults,
    onEdit: handleEdit,
    onValueChange: setValue,
    onVisibleChange: setVisible,
    onSave: handleSave,
    onCancel: handleCancelEdit,
    onRemove: handleRemove,
    onProbe: handleProbe,
    onRefreshModels: handleRefreshModels,
  };

  const localDataProps: ComponentProps<typeof SettingsLocalDataSection> = {
    sessionCount,
    cleared,
    onClearAll: handleClearAll,
  };

  return {
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
    providerLabel: getProviderLabel(selectedProvider, providers),
    modelLabel: getModelLabel(selectedModel, providers),
    error: error ?? (queryError ? `密钥状态读取失败：${queryError}` : null),
    providerRowsProps,
    localDataProps,
  };
}
