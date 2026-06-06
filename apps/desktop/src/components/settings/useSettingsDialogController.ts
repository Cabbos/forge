import { useCallback, useEffect, useState } from "react";
import type { ComponentProps } from "react";
import { useApiKeyStatusQuery } from "@/hooks/queries/useApiKeyStatusQuery";
import { SettingsLocalDataSection } from "@/components/settings/SettingsLocalDataSection";
import { SettingsProviderRows } from "@/components/settings/SettingsProviderRows";
import { buildSettingsProviderState } from "@/components/settings/SettingsDialogModel";
import { useSettingsDialogMotion } from "@/components/settings/useSettingsDialogMotion";
import { deleteSession, setApiKey } from "@/lib/tauri";
import { queryClient } from "@/lib/query-client";
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
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);
  const sessions = useStore((s) => s.sessions);
  const removeSession = useStore((s) => s.removeSession);
  const dialogOpen = open ?? internalOpen;
  const setDialogOpen = useCallback((nextOpen: boolean) => {
    if (open === undefined) setInternalOpen(nextOpen);
    onOpenChange?.(nextOpen);
  }, [onOpenChange, open]);
  const dialogRef = useSettingsDialogMotion(dialogOpen);
  const { data: keys = [] } = useApiKeyStatusQuery(dialogOpen);

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
    setCleared(true);
    setTimeout(() => setCleared(false), 3000);
  }, [removeSession, sessions]);

  const handleSave = useCallback(async () => {
    if (!editing) return;
    setSaving(true);
    setError(null);
    try {
      await setApiKey(editing, value);
      setEditing(null);
      setValue("");
      await queryClient.invalidateQueries({ queryKey: ["api-key-status"] });
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  }, [editing, value]);

  const handleRemove = useCallback(async (provider: string) => {
    setSaving(true);
    setError(null);
    try {
      await setApiKey(provider, "");
      await queryClient.invalidateQueries({ queryKey: ["api-key-status"] });
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  }, []);

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

  const { sortedKeys, configuredCount, providerTotal } = buildSettingsProviderState(keys);
  const sessionCount = sessions.size;

  const providerRowsProps: ComponentProps<typeof SettingsProviderRows> = {
    keys: sortedKeys,
    editing,
    value,
    visible,
    saving,
    onEdit: handleEdit,
    onValueChange: setValue,
    onVisibleChange: setVisible,
    onSave: handleSave,
    onCancel: handleCancelEdit,
    onRemove: handleRemove,
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
    configuredCount,
    providerTotal,
    sessionCount,
    error,
    providerRowsProps,
    localDataProps,
  };
}
